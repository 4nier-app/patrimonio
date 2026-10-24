//! Leitura mínima de planilha Excel (.xlsx): só os valores da primeira aba,
//! como texto. Basta para os extratos da Área do Investidor da B3, sem
//! depender de biblioteca de planilhas.

use std::io::{Cursor, Read};

use crate::{ProviderError, Result};

fn fmt(e: impl std::fmt::Display) -> ProviderError {
    ProviderError::Format(format!("planilha Excel inválida: {e}"))
}

fn read_entry(zip: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str) -> Result<Option<String>> {
    let mut f = match zip.by_name(name) {
        Ok(f) => f,
        Err(zip::result::ZipError::FileNotFound) => return Ok(None),
        Err(e) => return Err(fmt(e)),
    };
    let mut s = String::new();
    f.read_to_string(&mut s).map_err(fmt)?;
    Ok(Some(s))
}

/// Desfaz as entidades do XML (&amp; &lt; &gt; &quot; &apos; e &#NNN;).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let Some(end) = rest.find(';') else { break };
        let ent = &rest[1..end];
        let ch = match ent {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if ent.starts_with("#x") => u32::from_str_radix(&ent[2..], 16)
                .ok()
                .and_then(char::from_u32),
            _ if ent.starts_with('#') => ent[1..].parse().ok().and_then(char::from_u32),
            _ => None,
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// Texto de todas as tags <t> dentro de `xml` (um item de texto pode vir em
/// pedaços com formatação diferente).
fn texts(xml: &str) -> String {
    let mut out = String::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<t") {
        let after = &rest[i + 2..];
        // <t> ou <t xml:space="preserve">, mas não <tag...>.
        if !(after.starts_with('>') || after.starts_with(' ')) {
            rest = after;
            continue;
        }
        let Some(open_end) = after.find('>') else {
            break;
        };
        if after[..open_end].ends_with('/') {
            rest = &after[open_end + 1..];
            continue;
        }
        let body = &after[open_end + 1..];
        let Some(close) = body.find("</t>") else {
            break;
        };
        out.push_str(&unescape(&body[..close]));
        rest = &body[close + 4..];
    }
    out
}

/// Valor de um atributo dentro da tag de abertura.
fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!(" {name}=\"");
    let i = tag.find(&key)? + key.len();
    let end = tag[i..].find('"')?;
    Some(&tag[i..i + end])
}

/// Coluna de uma referência como "C12" (A = 0).
fn column(reference: &str) -> usize {
    reference
        .chars()
        .take_while(|c| c.is_ascii_alphabetic())
        .fold(0, |n, c| {
            n * 26 + (c.to_ascii_uppercase() as usize - 'A' as usize + 1)
        })
        .saturating_sub(1)
}

/// Linhas da primeira aba, cada célula como texto (números como gravados no
/// arquivo, com ponto decimal). Células vazias viram "".
pub fn first_sheet(bytes: &[u8]) -> Result<Vec<Vec<String>>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(fmt)?;
    let shared: Vec<String> = match read_entry(&mut zip, "xl/sharedStrings.xml")? {
        Some(xml) => xml
            .split("<si>")
            .skip(1)
            .map(|si| texts(si.split("</si>").next().unwrap_or("")))
            .collect(),
        None => Vec::new(),
    };
    let sheet = read_entry(&mut zip, "xl/worksheets/sheet1.xml")?
        .ok_or_else(|| fmt("sem a primeira aba"))?;
    let mut rows = Vec::new();
    for row in sheet.split("<row").skip(1) {
        let row = row.split("</row>").next().unwrap_or("");
        let mut cells: Vec<String> = Vec::new();
        for cell in row.split("<c").skip(1) {
            // "<c" também começa "<col"; células têm espaço ou ">" depois.
            if !(cell.starts_with(' ') || cell.starts_with('>') || cell.starts_with('/')) {
                continue;
            }
            let Some(tag_end) = cell.find('>') else {
                continue;
            };
            let tag = &cell[..tag_end];
            let col = attr(tag, "r").map(column).unwrap_or(cells.len());
            let kind = attr(tag, "t").unwrap_or("n");
            let body = if tag.ends_with('/') {
                ""
            } else {
                cell[tag_end + 1..].split("</c>").next().unwrap_or("")
            };
            let raw = body
                .split("<v>")
                .nth(1)
                .and_then(|v| v.split("</v>").next())
                .map(unescape);
            let value = match kind {
                "s" => raw
                    .and_then(|i| i.trim().parse::<usize>().ok())
                    .and_then(|i| shared.get(i).cloned())
                    .unwrap_or_default(),
                "inlineStr" => texts(body),
                _ => raw.unwrap_or_default(),
            };
            if cells.len() <= col {
                cells.resize(col + 1, String::new());
            }
            cells[col] = value;
        }
        rows.push(cells);
    }
    Ok(rows)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// Planilha mínima com texto compartilhado e números, para os testes.
    pub(crate) fn build(rows: &[Vec<&str>]) -> Vec<u8> {
        let mut shared: Vec<String> = Vec::new();
        let mut sheet = String::from("<worksheet><sheetData>");
        for (r, row) in rows.iter().enumerate() {
            sheet.push_str(&format!("<row r=\"{}\">", r + 1));
            for (c, v) in row.iter().enumerate() {
                let reference = format!("{}{}", (b'A' + c as u8) as char, r + 1);
                if v.parse::<f64>().is_ok() {
                    sheet.push_str(&format!("<c r=\"{reference}\" s=\"4\"><v>{v}</v></c>"));
                } else {
                    let i = shared.iter().position(|s| s == v).unwrap_or_else(|| {
                        shared.push(v.to_string());
                        shared.len() - 1
                    });
                    sheet.push_str(&format!("<c r=\"{reference}\" t=\"s\"><v>{i}</v></c>"));
                }
            }
            sheet.push_str("</row>");
        }
        sheet.push_str("</sheetData></worksheet>");
        let sst: String = shared
            .iter()
            .map(|s| {
                format!(
                    "<si><t xml:space=\"preserve\">{}</t></si>",
                    s.replace('&', "&amp;")
                )
            })
            .collect();
        let mut buf = Cursor::new(Vec::new());
        {
            let mut z = zip::ZipWriter::new(&mut buf);
            let opt = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            z.start_file("xl/sharedStrings.xml", opt).unwrap();
            z.write_all(format!("<sst>{sst}</sst>").as_bytes()).unwrap();
            z.start_file("xl/worksheets/sheet1.xml", opt).unwrap();
            z.write_all(sheet.as_bytes()).unwrap();
            z.finish().unwrap();
        }
        buf.into_inner()
    }

    #[test]
    fn le_textos_numeros_e_celulas_vazias() {
        let bytes = build(&[vec!["Nome", "Valor"], vec!["A & B", "12.5"]]);
        let rows = first_sheet(&bytes).unwrap();
        assert_eq!(rows[0], vec!["Nome", "Valor"]);
        assert_eq!(rows[1], vec!["A & B", "12.5"]);
        assert_eq!(column("C12"), 2);
        assert_eq!(column("AA1"), 26);
        assert_eq!(unescape("x&#233;y"), "xéy");
        assert!(first_sheet(b"nao e zip").is_err());
    }
}
