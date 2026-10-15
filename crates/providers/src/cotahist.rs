//! Séries históricas da B3 (arquivo COTAHIST, layout de largura fixa).
//!
//! Layout publicado pela B3 em "SeriesHistoricas_Layout.pdf". Cada linha tem
//! 245 caracteres; o registro 01 é uma cotação de um papel num pregão. Preços
//! vêm em centavos, sem vírgula, e devem ser divididos pelo fator de cotação.

use std::io::Read;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "B3-COTAHIST";
const BASE_URL: &str = "https://bvmf.bmfbovespa.com.br/InstDados/SerHist";

/// Arquivo diário de um pregão.
pub fn daily_url(date: NaiveDate) -> String {
    format!("{BASE_URL}/COTAHIST_D{}.ZIP", date.format("%d%m%Y"))
}

/// Arquivo com todos os pregões de um mês.
pub fn monthly_url(year: i32, month: u32) -> String {
    format!("{BASE_URL}/COTAHIST_M{month:02}{year}.ZIP")
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Record {
    pub date: NaiveDate,
    pub bdi: String,
    pub ticker: String,
    /// 010 = à vista, 020 = fracionário.
    pub market: String,
    pub short_name: String,
    pub specification: String,
    pub close: Decimal,
    pub isin: String,
}

impl Record {
    pub fn is_spot(&self) -> bool {
        self.market == "010"
    }

    /// Classificação usada no catálogo.
    pub fn kind(&self) -> &'static str {
        let spec = self.specification.trim();
        match self.bdi.as_str() {
            "12" => "FII",
            "14" if spec.starts_with("CI") => "ETF",
            _ if spec.starts_with("DR") => "BDR",
            "02" | "96"
                if spec.starts_with("ON") || spec.starts_with("PN") || spec.starts_with("UNT") =>
            {
                "ACAO"
            }
            _ => "OUTRO",
        }
    }
}

fn field(line: &str, from: usize, to: usize) -> Result<&str> {
    line.get(from..to)
        .ok_or_else(|| ProviderError::Format(format!("linha curta: {} caracteres", line.len())))
}

fn price(raw: &str, factor: &Decimal) -> Result<Decimal> {
    let cents: i64 = raw
        .trim()
        .parse()
        .map_err(|_| ProviderError::Format(format!("preço inválido: {raw}")))?;
    Ok(Decimal::new(cents, 2) / factor)
}

/// Interpreta uma linha. Retorna `None` para cabeçalho e rodapé.
pub fn parse_line(line: &str) -> Result<Option<Record>> {
    let line = line.trim_end_matches(['\r', '\n']);
    if !line.starts_with("01") {
        return Ok(None);
    }
    let date = NaiveDate::parse_from_str(field(line, 2, 10)?, "%Y%m%d")
        .map_err(|e| ProviderError::Format(format!("data inválida: {e}")))?;
    let factor: Decimal = field(line, 210, 217)?
        .trim()
        .parse::<i64>()
        .map(Decimal::from)
        .map_err(|_| ProviderError::Format("fator de cotação inválido".into()))?;
    if factor.is_zero() {
        return Err(ProviderError::Format("fator de cotação zero".into()));
    }
    Ok(Some(Record {
        date,
        bdi: field(line, 10, 12)?.to_string(),
        ticker: field(line, 12, 24)?.trim().to_string(),
        market: field(line, 24, 27)?.to_string(),
        short_name: field(line, 27, 39)?.trim().to_string(),
        specification: field(line, 39, 49)?.trim().to_string(),
        close: price(field(line, 108, 121)?, &factor)?,
        isin: field(line, 230, 242)?.trim().to_string(),
    }))
}

/// Interpreta o texto completo (ISO-8859-1) do arquivo.
pub fn parse_text(bytes: &[u8]) -> Result<Vec<Record>> {
    // ISO-8859-1 → Unicode é byte a byte.
    let text: String = bytes.iter().map(|&b| b as char).collect();
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(r) = parse_line(line)? {
            out.push(r);
        }
    }
    Ok(out)
}

/// Extrai o primeiro arquivo de dentro do ZIP.
pub fn unzip_first(zip_bytes: &[u8]) -> Result<Vec<u8>> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|e| ProviderError::Format(format!("zip inválido: {e}")))?;
    let mut file = archive
        .by_index(0)
        .map_err(|e| ProviderError::Format(format!("zip vazio: {e}")))?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| ProviderError::Format(e.to_string()))?;
    Ok(buf)
}

/// Baixa e interpreta um arquivo COTAHIST.
pub fn fetch(url: &str) -> Result<Vec<Record>> {
    let zipped = http::get_bytes(url)?;
    parse_text(&unzip_first(&zipped)?)
}

/// Último pregão disponível: tenta os arquivos diários de `today` para trás.
pub fn fetch_latest_daily(
    today: NaiveDate,
    max_days_back: i64,
) -> Result<(NaiveDate, Vec<Record>)> {
    let mut last_err = ProviderError::NotFound("nenhum pregão recente".into());
    for back in 0..=max_days_back {
        let d = today - chrono::Duration::days(back);
        match fetch(&daily_url(d)) {
            Ok(records) if !records.is_empty() => return Ok((d, records)),
            Ok(_) => {}
            Err(e @ ProviderError::Network(_)) => return Err(e),
            Err(e) => last_err = e,
        }
    }
    Err(last_err)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    /// Monta uma linha sintética no layout oficial.
    pub fn line(
        date: &str,
        bdi: &str,
        ticker: &str,
        market: &str,
        name: &str,
        spec: &str,
        close_cents: i64,
        factor: i64,
    ) -> String {
        let mut s = String::new();
        s.push_str("01");
        s.push_str(date);
        s.push_str(bdi);
        s.push_str(&format!("{ticker:<12}"));
        s.push_str(market);
        s.push_str(&format!("{name:<12}"));
        s.push_str(&format!("{spec:<10}"));
        s.push_str(&" ".repeat(3 + 4)); // PRAZOT, MODREF
        for _ in 0..4 {
            s.push_str(&format!("{:013}", 0)); // abertura, máxima, mínima, média
        }
        s.push_str(&format!("{close_cents:013}"));
        s.push_str(&format!("{:013}{:013}", 0, 0)); // melhores ofertas
        s.push_str(&format!("{:05}{:018}{:018}", 0, 0, 0));
        s.push_str(&format!("{:013}", 0)); // PREEXE
        s.push('0'); // INDOPC
        s.push_str("99991231");
        s.push_str(&format!("{factor:07}"));
        s.push_str(&format!("{:013}", 0)); // PTOEXE
        s.push_str("BRTESTACNOR0");
        s.push_str("100");
        assert_eq!(s.len(), 245);
        s
    }

    #[test]
    fn interpreta_registro_e_ignora_cabecalho() {
        let text = [
            "00COTAHIST.2026BOVESPA 20260302".to_string(),
            line(
                "20260302",
                "02",
                "TEST3",
                "010",
                "EMPRESA TES",
                "ON NM",
                1234,
                1,
            ),
            line(
                "20260302",
                "02",
                "TEST3F",
                "020",
                "EMPRESA TES",
                "ON NM",
                1235,
                1,
            ),
            line(
                "20260302",
                "12",
                "FUND11",
                "010",
                "FUNDO TESTE",
                "CI",
                9850,
                1,
            ),
            line(
                "20260302",
                "14",
                "INDX11",
                "010",
                "ETF TESTE",
                "CI",
                10012,
                1,
            ),
            line(
                "20260302", "02", "MIL3", "010", "COTA MIL", "ON", 4500, 1000,
            ),
            "99COTAHIST.2026BOVESPA 2026030200000000005".to_string(),
        ]
        .join("\r\n");
        let recs = parse_text(text.as_bytes()).unwrap();
        assert_eq!(recs.len(), 5);
        assert_eq!(recs[0].ticker, "TEST3");
        assert_eq!(recs[0].close, dec!(12.34));
        assert_eq!(recs[0].date, NaiveDate::from_ymd_opt(2026, 3, 2).unwrap());
        assert_eq!(recs[0].kind(), "ACAO");
        assert!(!recs[1].is_spot());
        assert_eq!(recs[2].kind(), "FII");
        assert_eq!(recs[3].kind(), "ETF");
        assert_eq!(recs[4].close, dec!(0.045));
    }

    #[test]
    fn nomes_dos_arquivos() {
        let d = NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
        assert!(daily_url(d).ends_with("COTAHIST_D02032026.ZIP"));
        assert!(monthly_url(2026, 3).ends_with("COTAHIST_M032026.ZIP"));
    }

    #[test]
    fn linha_corrompida_e_erro() {
        assert!(parse_line("0120260302").is_err());
    }
}
