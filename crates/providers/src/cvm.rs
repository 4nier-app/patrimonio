//! Dados abertos da CVM sobre fundos de investimento.
//!
//! - Informe diário (dados.cvm.gov.br/dados/FI/DOC/INF_DIARIO): valor da cota
//!   de cada fundo em cada dia útil. Um ZIP por mês desde 2021 e um por ano
//!   até 2020 (pasta HIST). Os arquivos são grandes (10 MB por mês, até
//!   90 MB por ano), então são lidos linha a linha, guardando só os fundos
//!   da carteira.
//! - Cadastro (dados.cvm.gov.br/dados/FI/CAD): CNPJ e nome de cada fundo,
//!   para achar o fundo pelo nome que aparece no extrato do banco.
//!
//! Os arquivos são CSV separados por ";" em ISO-8859-1. As colunas são
//! achadas pelo nome do cabeçalho, porque mudaram com a Resolução CVM 175
//! (CNPJ_FUNDO virou CNPJ_FUNDO_CLASSE).

use std::collections::{BTreeMap, BTreeSet};
use std::io::{BufRead, BufReader, Read};

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "CVM";
pub const CATALOG_SOURCE: &str = "CVM-CADASTRO";
const BASE: &str = "https://dados.cvm.gov.br/dados/FI";

/// Primeiro ano com arquivo mensal; antes dele, um arquivo por ano.
pub const FIRST_MONTHLY_YEAR: i32 = 2021;

pub fn monthly_url(year: i32, month: u32) -> String {
    format!("{BASE}/DOC/INF_DIARIO/DADOS/inf_diario_fi_{year}{month:02}.zip")
}

pub fn yearly_url(year: i32) -> String {
    format!("{BASE}/DOC/INF_DIARIO/DADOS/HIST/inf_diario_fi_{year}.zip")
}

pub const CAD_FI_URL: &str = "https://dados.cvm.gov.br/dados/FI/CAD/DADOS/cad_fi.csv";
pub const REGISTRO_URL: &str =
    "https://dados.cvm.gov.br/dados/FI/CAD/DADOS/registro_fundo_classe.zip";

/// Cota de um fundo num dia. `cnpj` são os 14 dígitos.
#[derive(Debug, Clone, PartialEq)]
pub struct DailyQuota {
    pub cnpj: String,
    pub date: NaiveDate,
    pub quota: Decimal,
}

/// Fundo do cadastro.
#[derive(Debug, Clone, PartialEq)]
pub struct FundEntry {
    pub cnpj: String,
    pub name: String,
    pub cancelled: bool,
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

/// Lê um CSV linha a linha, entregando cada linha já separada.
struct Csv<R> {
    reader: R,
    sep: char,
    header: Vec<String>,
    buf: Vec<u8>,
}

impl<R: BufRead> Csv<R> {
    fn new(mut reader: R) -> Result<Self> {
        let mut buf = Vec::new();
        reader
            .read_until(b'\n', &mut buf)
            .map_err(|e| ProviderError::Format(format!("leitura: {e}")))?;
        let line = latin1(&buf);
        let line = line
            .trim_start_matches('\u{feff}')
            .trim_start_matches("ï»¿");
        let sep = if line.matches(';').count() >= line.matches(',').count() {
            ';'
        } else {
            ','
        };
        let header = line
            .trim_end_matches(['\r', '\n'])
            .split(sep)
            .map(|h| h.trim().trim_matches('"').to_uppercase())
            .collect();
        Ok(Csv {
            reader,
            sep,
            header,
            buf,
        })
    }

    /// Posição da primeira coluna com um destes nomes.
    fn column(&self, names: &[&str]) -> Option<usize> {
        names
            .iter()
            .find_map(|n| self.header.iter().position(|h| h == n))
    }

    fn next_row(&mut self) -> Result<Option<Vec<String>>> {
        self.buf.clear();
        let n = self
            .reader
            .read_until(b'\n', &mut self.buf)
            .map_err(|e| ProviderError::Format(format!("leitura: {e}")))?;
        if n == 0 {
            return Ok(None);
        }
        let line = latin1(&self.buf);
        Ok(Some(
            line.trim_end_matches(['\r', '\n'])
                .split(self.sep)
                .map(|f| f.trim().trim_matches('"').to_string())
                .collect(),
        ))
    }
}

fn parse_decimal(s: &str) -> Option<Decimal> {
    let s = s.trim();
    let s = if s.contains(',') && !s.contains('.') {
        s.replace(',', ".")
    } else {
        s.to_string()
    };
    s.parse().ok()
}

/// Cotas dos fundos pedidos (CNPJ só com dígitos) num informe diário.
/// Quando o fundo tem subclasses, vale a linha da classe (subclasse vazia).
pub fn parse_daily(reader: impl BufRead, wanted: &BTreeSet<String>) -> Result<Vec<DailyQuota>> {
    let mut csv = Csv::new(reader)?;
    let cnpj_col = csv
        .column(&["CNPJ_FUNDO_CLASSE", "CNPJ_FUNDO", "CNPJ_CLASSE"])
        .ok_or_else(|| ProviderError::Format("informe diário sem coluna de CNPJ".into()))?;
    let date_col = csv
        .column(&["DT_COMPTC"])
        .ok_or_else(|| ProviderError::Format("informe diário sem DT_COMPTC".into()))?;
    let quota_col = csv
        .column(&["VL_QUOTA"])
        .ok_or_else(|| ProviderError::Format("informe diário sem VL_QUOTA".into()))?;
    let sub_col = csv.column(&["ID_SUBCLASSE"]);
    // (cnpj, data) → (é da classe, cota)
    let mut found: BTreeMap<(String, NaiveDate), (bool, Decimal)> = BTreeMap::new();
    while let Some(row) = csv.next_row()? {
        let Some(raw) = row.get(cnpj_col) else {
            continue;
        };
        let cnpj = digits(raw);
        if !wanted.contains(&cnpj) {
            continue;
        }
        let (Some(d), Some(q)) = (row.get(date_col), row.get(quota_col)) else {
            continue;
        };
        let Ok(date) = NaiveDate::parse_from_str(d, "%Y-%m-%d") else {
            continue;
        };
        let Some(quota) = parse_decimal(q).filter(|q| *q > Decimal::ZERO) else {
            continue;
        };
        let class_level = sub_col
            .and_then(|c| row.get(c))
            .is_none_or(|s| s.is_empty());
        let entry = found.entry((cnpj, date)).or_insert((class_level, quota));
        if class_level && !entry.0 {
            *entry = (true, quota);
        }
    }
    Ok(found
        .into_iter()
        .map(|((cnpj, date), (_, quota))| DailyQuota {
            cnpj,
            date,
            quota: quota.normalize(),
        })
        .collect())
}

/// Lê todos os CSV de dentro do ZIP (o anual pode trazer um por mês).
pub fn parse_daily_zip(zip_bytes: &[u8], wanted: &BTreeSet<String>) -> Result<Vec<DailyQuota>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| ProviderError::Format(format!("zip inválido: {e}")))?;
    let mut out = Vec::new();
    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| ProviderError::Format(format!("zip: {e}")))?;
        if !file.name().to_lowercase().ends_with(".csv") {
            continue;
        }
        out.extend(parse_daily(BufReader::new(file), wanted)?);
    }
    Ok(out)
}

/// Baixa um informe diário (mensal ou anual) e devolve as cotas pedidas.
pub fn fetch_daily(url: &str, wanted: &BTreeSet<String>) -> Result<Vec<DailyQuota>> {
    let zipped = http::get_bytes(url)?;
    parse_daily_zip(&zipped, wanted)
}

/// Fundos de um arquivo de cadastro (cad_fi.csv, registro_fundo.csv ou
/// registro_classe.csv).
pub fn parse_catalog(reader: impl BufRead) -> Result<Vec<FundEntry>> {
    let mut csv = Csv::new(reader)?;
    let cnpj_col = csv
        .column(&["CNPJ_CLASSE", "CNPJ_FUNDO_CLASSE", "CNPJ_FUNDO"])
        .ok_or_else(|| ProviderError::Format("cadastro sem coluna de CNPJ".into()))?;
    let name_col = csv
        .column(&["DENOMINACAO_SOCIAL", "DENOM_SOCIAL"])
        .ok_or_else(|| ProviderError::Format("cadastro sem coluna de nome".into()))?;
    let sit_col = csv.column(&["SITUACAO", "SIT"]);
    let mut out = Vec::new();
    while let Some(row) = csv.next_row()? {
        let cnpj = row.get(cnpj_col).map(|s| digits(s)).unwrap_or_default();
        let name = row.get(name_col).map(|s| s.trim()).unwrap_or_default();
        if cnpj.len() != 14 || name.is_empty() {
            continue;
        }
        let cancelled = sit_col
            .and_then(|c| row.get(c))
            .is_some_and(|s| s.to_uppercase().starts_with("CANCEL"));
        out.push(FundEntry {
            cnpj,
            name: name.split_whitespace().collect::<Vec<_>>().join(" "),
            cancelled,
        });
    }
    Ok(out)
}

/// Junta cadastros: um fundo por CNPJ, preferindo o registro em
/// funcionamento e, entre iguais, o que veio por último (mais novo).
pub fn merge_catalogs(lists: Vec<Vec<FundEntry>>) -> Vec<FundEntry> {
    let mut by_cnpj: BTreeMap<String, FundEntry> = BTreeMap::new();
    for e in lists.into_iter().flatten() {
        match by_cnpj.get(&e.cnpj) {
            Some(old) if !old.cancelled && e.cancelled => {}
            _ => {
                by_cnpj.insert(e.cnpj.clone(), e);
            }
        }
    }
    by_cnpj.into_values().collect()
}

/// Baixa o cadastro antigo (cad_fi.csv) e o novo (registro de fundos e
/// classes). Basta um dos dois responder.
pub fn fetch_catalog() -> Result<Vec<FundEntry>> {
    let mut lists = Vec::new();
    let mut errors = Vec::new();
    match http::get_bytes(CAD_FI_URL).and_then(|b| parse_catalog(&b[..])) {
        Ok(l) => lists.push(l),
        Err(e) => errors.push(e),
    }
    match http::get_bytes(REGISTRO_URL).and_then(|b| parse_registro_zip(&b)) {
        Ok(l) => lists.extend(l),
        Err(e) => errors.push(e),
    }
    if lists.is_empty() {
        return Err(errors
            .into_iter()
            .next()
            .unwrap_or_else(|| ProviderError::Format("cadastro vazio".into())));
    }
    Ok(merge_catalogs(lists))
}

/// registro_fundo.csv antes de registro_classe.csv: a classe, mais nova,
/// prevalece no mesmo CNPJ.
pub fn parse_registro_zip(zip_bytes: &[u8]) -> Result<Vec<Vec<FundEntry>>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| ProviderError::Format(format!("zip inválido: {e}")))?;
    let mut lists = Vec::new();
    for wanted in ["registro_fundo", "registro_classe"] {
        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| ProviderError::Format(format!("zip: {e}")))?;
            let name = file.name().to_lowercase();
            if !name.contains(wanted) || !name.ends_with(".csv") {
                continue;
            }
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|e| ProviderError::Format(format!("zip: {e}")))?;
            // Arquivo sem as colunas esperadas é ignorado, não derruba o resto.
            if let Ok(l) = parse_catalog(&bytes[..]) {
                lists.push(l);
            }
        }
    }
    Ok(lists)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;
    use std::io::Write;

    // Dados fictícios, no formato publicado pela CVM.
    const DAILY_NOVO: &str = "TP_FUNDO_CLASSE;CNPJ_FUNDO_CLASSE;ID_SUBCLASSE;DT_COMPTC;VL_TOTAL;VL_QUOTA;VL_PATRIM_LIQ;CAPTC_DIA;RESG_DIA;NR_COTST\r\n\
CLASSES - FIF;11.222.333/0001-81;;2026-09-01;1000.00;12.309876500000;1000.00;0.00;0.00;10\r\n\
CLASSES - FIF;11.222.333/0001-81;SUB1;2026-09-02;1000.00;99.000000000000;1000.00;0.00;0.00;10\r\n\
CLASSES - FIF;11.222.333/0001-81;;2026-09-02;1000.00;12.400000000000;1000.00;0.00;0.00;10\r\n\
CLASSES - FIF;99.888.777/0001-00;;2026-09-01;1000.00;1.000000000000;1000.00;0.00;0.00;10\r\n";

    const DAILY_ANTIGO: &str = "TP_FUNDO,CNPJ_FUNDO,DT_COMPTC,VL_TOTAL,VL_QUOTA,VL_PATRIM_LIQ,CAPTC_DIA,RESG_DIA,NR_COTST\n\
FI,11.444.777/0001-61,2018-03-01,1000.00,2.5,1000.00,0,0,1\n";

    #[test]
    fn le_o_informe_diario_pelo_cabecalho() {
        let wanted: BTreeSet<String> = ["11222333000181".to_string()].into();
        let q = parse_daily(DAILY_NOVO.as_bytes(), &wanted).unwrap();
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].quota, dec!(12.3098765));
        assert_eq!(q[1].date, NaiveDate::from_ymd_opt(2026, 9, 2).unwrap());
        assert_eq!(q[1].quota, dec!(12.4), "a classe vale mais que a subclasse");

        let wanted: BTreeSet<String> = ["11444777000161".to_string()].into();
        let q = parse_daily(DAILY_ANTIGO.as_bytes(), &wanted).unwrap();
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].quota, dec!(2.5));
    }

    #[test]
    fn le_todos_os_csv_do_zip() {
        let mut buf = std::io::Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut buf);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            w.start_file("inf_diario_fi_201801.csv", opts).unwrap();
            w.write_all(DAILY_ANTIGO.as_bytes()).unwrap();
            w.start_file("inf_diario_fi_202609.csv", opts).unwrap();
            w.write_all(DAILY_NOVO.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        let wanted: BTreeSet<String> =
            ["11444777000161".to_string(), "11222333000181".to_string()].into();
        let q = parse_daily_zip(buf.get_ref(), &wanted).unwrap();
        assert_eq!(q.len(), 3);
        assert!(parse_daily_zip(b"nada", &wanted).is_err());
    }

    #[test]
    fn cadastro_com_nome_e_situacao() {
        let cad = "TP_FUNDO;CNPJ_FUNDO;DENOM_SOCIAL;DT_REG;SIT\n\
FI;11.222.333/0001-81;FUNDO TESTE  ACOES;2010-01-01;CANCELADA\n\
FI;11.444.777/0001-61;FUNDO TESTE DI;2010-01-01;EM FUNCIONAMENTO NORMAL\n";
        let classe = "ID_Registro_Fundo;ID_Registro_Classe;CNPJ_Classe;Codigo_CVM;Denominacao_Social;Situacao\n\
1;2;11.222.333/0001-81;3;FUNDO TESTE ACOES - CLASSE UNICA;Em Funcionamento Normal\n\
1;3;11.444.777/0001-61;4;FUNDO TESTE DI ANTIGO;Cancelado\n";
        let a = parse_catalog(cad.as_bytes()).unwrap();
        assert_eq!(a[0].name, "FUNDO TESTE ACOES");
        assert!(a[0].cancelled);
        let b = parse_catalog(classe.as_bytes()).unwrap();
        let m = merge_catalogs(vec![a, b]);
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].name, "FUNDO TESTE ACOES - CLASSE UNICA");
        assert!(!m[0].cancelled);
        assert_eq!(
            m[1].name, "FUNDO TESTE DI",
            "o ativo vale mais que o cancelado"
        );
    }

    #[test]
    fn nomes_dos_arquivos() {
        assert!(monthly_url(2026, 9).ends_with("/INF_DIARIO/DADOS/inf_diario_fi_202609.zip"));
        assert!(yearly_url(2019).ends_with("/INF_DIARIO/DADOS/HIST/inf_diario_fi_2019.zip"));
    }
}
