//! Tesouro Direto: títulos públicos lançados à mão (compra e venda em
//! quantidade de títulos, com o preço unitário do dia) e cotados pelo preço
//! que o Tesouro publica todo dia útil nos dados abertos.
//!
//! O código do título na carteira é "TD:<sigla>:<vencimento AAAAMMDD>", por
//! exemplo "TD:NTNB-P:20350515" para o Tesouro IPCA+ 2035. Como nos fundos,
//! não há taxas da B3 nem DARF: o IR e o IOF são retidos no resgate e
//! informados no lançamento.

use chrono::NaiveDate;

pub const KIND: &str = "TESOURO";
const PREFIX: &str = "TD:";

/// Sigla oficial do título pelo nome do tipo, como aparece nos dados abertos.
pub fn code_of(kind: &str) -> String {
    let k = kind.trim().to_lowercase();
    let semestral = k.contains("juros semestrais");
    let code = if k.contains("selic") {
        "LFT"
    } else if k.contains("prefixado") {
        if semestral {
            "NTN-F"
        } else {
            "LTN"
        }
    } else if k.contains("igpm") {
        "NTN-C"
    } else if k.contains("renda+") {
        "RENDA"
    } else if k.contains("educa+") {
        "EDUCA"
    } else if k.contains("ipca") {
        if semestral {
            "NTN-B"
        } else {
            "NTN-B-P"
        }
    } else {
        ""
    };
    if !code.is_empty() {
        return code.to_string();
    }
    k.to_uppercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Código do título na carteira.
pub fn ticker(kind: &str, maturity: NaiveDate) -> String {
    format!("{PREFIX}{}:{}", code_of(kind), maturity.format("%Y%m%d"))
}

/// O código é de um título do Tesouro Direto?
pub fn is_tesouro(ticker: &str) -> bool {
    ticker.starts_with(PREFIX)
}

/// Vencimento do título pelo código.
pub fn maturity_of(ticker: &str) -> Option<NaiveDate> {
    let date = ticker.strip_prefix(PREFIX)?.rsplit(':').next()?;
    NaiveDate::parse_from_str(date, "%Y%m%d").ok()
}

/// Nome para a tela: "Tesouro IPCA+ 2035".
pub fn display_name(kind: &str, maturity: NaiveDate) -> String {
    format!("{} {}", kind.trim(), maturity.format("%Y"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codigo_do_titulo() {
        let d = NaiveDate::from_ymd_opt(2035, 5, 15).unwrap();
        assert_eq!(ticker("Tesouro IPCA+", d), "TD:NTN-B-P:20350515");
        assert_eq!(
            ticker("Tesouro IPCA+ com Juros Semestrais", d),
            "TD:NTN-B:20350515"
        );
        assert_eq!(ticker("Tesouro Selic", d), "TD:LFT:20350515");
        assert_eq!(ticker("Tesouro Prefixado", d), "TD:LTN:20350515");
        assert_eq!(
            ticker("Tesouro Prefixado com Juros Semestrais", d),
            "TD:NTN-F:20350515"
        );
        assert_eq!(
            ticker("Tesouro Renda+ Aposentadoria Extra", d),
            "TD:RENDA:20350515"
        );
        assert!(is_tesouro("TD:LFT:20350515"));
        assert!(!is_tesouro("ABCD3"));
        assert_eq!(maturity_of("TD:NTN-B-P:20350515"), Some(d));
        assert_eq!(display_name("Tesouro IPCA+", d), "Tesouro IPCA+ 2035");
    }
}
