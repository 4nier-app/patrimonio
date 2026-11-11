//! Exporta tabelas das telas para uma planilha do Excel (.xlsx).
//!
//! A tela monta a tabela (o que aparece, na ordem em que aparece) e aqui só
//! se escreve o arquivo, com números de verdade para o Excel somar.

use rust_xlsxwriter::{Color, Format, FormatAlign, FormatBorder, Workbook, XlsxError};
use serde::Deserialize;
use std::path::Path;

/// Uma célula: texto, número com formato ou vazia.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Cell {
    Text(String),
    Number { n: f64, f: NumberKind },
    Date { d: String },
    Empty(()),
}

#[derive(Debug, Deserialize, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum NumberKind {
    Money,
    Pct,
    Qty,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RowStyle {
    Head,
    Group,
    Row,
    Total,
    Grand,
    Flow,
}

#[derive(Debug, Deserialize)]
pub struct Row {
    pub style: RowStyle,
    pub cells: Vec<Cell>,
}

#[derive(Debug, Deserialize)]
pub struct Sheet {
    /// Nome da aba (até 31 caracteres).
    pub name: String,
    pub title: String,
    pub note: String,
    /// Largura de cada coluna, em caracteres.
    pub widths: Vec<f64>,
    pub rows: Vec<Row>,
    /// Colunas que ficam paradas ao rolar para os lados.
    #[serde(default)]
    pub freeze_cols: u16,
    /// Filtro no cabeçalho (primeira linha de `rows`).
    #[serde(default)]
    pub filter: bool,
}

const NAVY: u32 = 0x0F2A44;
const TEAL: u32 = 0x1F8A84;
const LIGHT: u32 = 0xE8F3F2;
const GRAY: u32 = 0xF3F4F6;
const MUTED: u32 = 0x6B7280;
const FIRST: u32 = 3;

fn base(style: RowStyle) -> Format {
    let f = Format::new()
        .set_border_bottom(FormatBorder::Thin)
        .set_border_bottom_color(Color::RGB(0xD0D7DE));
    match style {
        RowStyle::Head => f
            .set_bold()
            .set_font_color(Color::White)
            .set_background_color(Color::RGB(NAVY))
            .set_align(FormatAlign::Center)
            .set_text_wrap(),
        RowStyle::Group => f.set_bold().set_background_color(Color::RGB(GRAY)),
        RowStyle::Total => f.set_bold().set_background_color(Color::RGB(LIGHT)),
        RowStyle::Grand => f
            .set_bold()
            .set_font_color(Color::White)
            .set_background_color(Color::RGB(TEAL)),
        RowStyle::Flow => f.set_italic().set_font_color(Color::RGB(MUTED)),
        RowStyle::Row => f,
    }
}

fn number_format(f: Format, kind: NumberKind, white: bool) -> Format {
    let red = if white { "" } else { "[Red]" };
    match kind {
        NumberKind::Money => f.set_num_format(format!("#,##0.00;{red}-#,##0.00")),
        NumberKind::Pct => f.set_num_format(format!("0.00%;{red}-0.00%")),
        NumberKind::Qty => f.set_num_format("#,##0.########"),
    }
}

/// Grava as abas num arquivo .xlsx.
pub fn write(sheets: &[Sheet], path: &Path) -> Result<(), XlsxError> {
    let mut book = Workbook::new();
    for s in sheets {
        let ws = book.add_worksheet();
        let name: String = s
            .name
            .chars()
            .filter(|c| !"[]:*?/\\".contains(*c))
            .take(31)
            .collect();
        ws.set_name(name)?;
        ws.write_string_with_format(
            0,
            0,
            &s.title,
            &Format::new()
                .set_bold()
                .set_font_size(14)
                .set_font_color(Color::RGB(NAVY)),
        )?;
        ws.write_string_with_format(
            1,
            0,
            &s.note,
            &Format::new().set_italic().set_font_color(Color::RGB(MUTED)),
        )?;
        for (c, w) in s.widths.iter().enumerate() {
            ws.set_column_width(c as u16, *w)?;
        }
        for (i, row) in s.rows.iter().enumerate() {
            let r = FIRST + i as u32;
            let fmt = base(row.style);
            let white = matches!(row.style, RowStyle::Head | RowStyle::Grand);
            for (c, cell) in row.cells.iter().enumerate() {
                let c = c as u16;
                match cell {
                    Cell::Text(t) => ws.write_string_with_format(r, c, t, &fmt)?,
                    Cell::Number { n, f } => ws.write_number_with_format(
                        r,
                        c,
                        *n,
                        &number_format(fmt.clone(), *f, white),
                    )?,
                    Cell::Date { d } => match chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d") {
                        Ok(day) => {
                            let excel = rust_xlsxwriter::ExcelDateTime::from_ymd(
                                chrono::Datelike::year(&day) as u16,
                                chrono::Datelike::month(&day) as u8,
                                chrono::Datelike::day(&day) as u8,
                            )?;
                            ws.write_datetime_with_format(
                                r,
                                c,
                                &excel,
                                &fmt.clone().set_num_format("dd/mm/yyyy"),
                            )?
                        }
                        Err(_) => ws.write_string_with_format(r, c, d, &fmt)?,
                    },
                    Cell::Empty(()) => ws.write_blank(r, c, &fmt)?,
                };
            }
        }
        if !s.rows.is_empty() {
            ws.set_freeze_panes(FIRST + 1, s.freeze_cols)?;
            if s.filter {
                let last = s
                    .rows
                    .iter()
                    .map(|r| r.cells.len())
                    .max()
                    .unwrap_or(1)
                    .saturating_sub(1) as u16;
                ws.autofilter(FIRST, 0, FIRST + s.rows.len() as u32 - 1, last)?;
            }
        }
    }
    book.save(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_planilha_com_numeros_datas_e_vazios() {
        let json = r#"[{"name":"Lançamentos","title":"T","note":"N","widths":[12,10,10],"filter":true,
            "rows":[{"style":"head","cells":["Data","Ativo","Valor"]},
                    {"style":"row","cells":[{"d":"2026-10-03"},"WEGE3",{"n":-4120.5,"f":"money"}]},
                    {"style":"total","cells":["Total",null,{"n":0.0123,"f":"pct"}]}]}]"#;
        let sheets: Vec<Sheet> = serde_json::from_str(json).unwrap();
        assert!(matches!(sheets[0].rows[2].cells[1], Cell::Empty(())));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.xlsx");
        write(&sheets, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..2], b"PK");
    }
}
