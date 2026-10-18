//! Tesouro Direto (dados abertos do Tesouro Transparente): preço e taxa de
//! cada título em cada dia útil, desde 2002, num CSV só.
//!
//! O arquivo tem cerca de 15 MB e é separado por ";", com vírgula decimal e
//! datas dd/mm/aaaa. As colunas são achadas pelo nome do cabeçalho. O preço
//! usado é o "PU Venda Manhã" (quanto o Tesouro paga para recomprar o título,
//! o valor do extrato); quando ele vem zerado, o "PU Base Manhã".

use std::collections::BTreeMap;

use chrono::NaiveDate;
use rust_decimal::Decimal;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "TESOURO";
pub const URL: &str = "https://www.tesourotransparente.gov.br/ckan/dataset/df56aa42-484a-4a59-8184-7676580c81e3/resource/796d2059-14e9-44e3-80c9-2d9e30b405c1/download/PrecoTaxaTesouroDireto.csv";

/// Um título que aparece no arquivo.
#[derive(Debug, Clone, PartialEq)]
pub struct Title {
    /// "Tesouro IPCA+", como no arquivo.
    pub kind: String,
    pub maturity: NaiveDate,
    /// Último dia com preço publicado.
    pub last_date: NaiveDate,
}

/// Preço de um título num dia.
#[derive(Debug, Clone, PartialEq)]
pub struct Price {
    pub kind: String,
    pub maturity: NaiveDate,
    pub date: NaiveDate,
    pub price: Decimal,
}

#[derive(Debug, Default)]
pub struct Data {
    pub titles: Vec<Title>,
    /// Só dos títulos pedidos.
    pub prices: Vec<Price>,
}

fn plain(s: &str) -> String {
    s.trim()
        .trim_start_matches('\u{feff}')
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'á' | 'à' | 'ã' | 'â' => 'a',
            'é' | 'ê' => 'e',
            'í' => 'i',
            'ó' | 'õ' | 'ô' => 'o',
            'ú' => 'u',
            'ç' => 'c',
            c => c,
        })
        .collect()
}

fn date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%d/%m/%Y").ok()
}

fn number(s: &str) -> Option<Decimal> {
    s.trim().replace('.', "").replace(',', ".").parse().ok()
}

fn text(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_string(),
        // Arquivos antigos vinham em ISO-8859-1.
        Err(_) => bytes.iter().map(|&b| b as char).collect(),
    }
}

/// Interpreta o CSV. `wanted(tipo, vencimento)`: títulos cujos preços
/// interessam.
pub fn parse(bytes: &[u8], wanted: &dyn Fn(&str, NaiveDate) -> bool) -> Result<Data> {
    let content = text(bytes);
    let mut lines = content.lines();
    let header: Vec<String> = lines
        .next()
        .ok_or_else(|| ProviderError::Format("arquivo vazio".into()))?
        .split(';')
        .map(plain)
        .collect();
    let col = |name: &str| header.iter().position(|h| h == name);
    let (Some(kind), Some(maturity), Some(base)) =
        (col("tipo titulo"), col("data vencimento"), col("data base"))
    else {
        return Err(ProviderError::Format(format!(
            "colunas do Tesouro não encontradas: {}",
            header.join(";")
        )));
    };
    let sell = col("pu venda manha");
    let pu_base = col("pu base manha");
    if sell.is_none() && pu_base.is_none() {
        return Err(ProviderError::Format(
            "coluna de preço não encontrada".into(),
        ));
    }
    let mut titles: BTreeMap<(String, NaiveDate), NaiveDate> = BTreeMap::new();
    let mut prices = Vec::new();
    for line in lines {
        let f: Vec<&str> = line.split(';').collect();
        let get = |i: usize| f.get(i).copied().unwrap_or("");
        let (Some(m), Some(d)) = (date(get(maturity)), date(get(base))) else {
            continue;
        };
        let k = get(kind).trim().to_string();
        if k.is_empty() {
            continue;
        }
        let last = titles.entry((k.clone(), m)).or_insert(d);
        if d > *last {
            *last = d;
        }
        if !wanted(&k, m) {
            continue;
        }
        let price = sell
            .and_then(|i| number(get(i)))
            .filter(|p| !p.is_zero())
            .or_else(|| pu_base.and_then(|i| number(get(i))))
            .filter(|p| !p.is_zero());
        if let Some(price) = price {
            prices.push(Price {
                kind: k,
                maturity: m,
                date: d,
                price,
            });
        }
    }
    if titles.is_empty() {
        return Err(ProviderError::Format("nenhum título no arquivo".into()));
    }
    Ok(Data {
        titles: titles
            .into_iter()
            .map(|((kind, maturity), last_date)| Title {
                kind,
                maturity,
                last_date,
            })
            .collect(),
        prices,
    })
}

pub fn fetch(wanted: &dyn Fn(&str, NaiveDate) -> bool) -> Result<Data> {
    parse(&http::get_bytes(URL)?, wanted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // Linhas fictícias no formato do arquivo.
    const SAMPLE: &str = "\u{feff}Tipo Titulo;Data Vencimento;Data Base;Taxa Compra Manha;Taxa Venda Manha;PU Compra Manha;PU Venda Manha;PU Base Manha\n\
Tesouro IPCA+;15/05/2035;02/01/2026;6,50;6,62;2100,10;2080,55;2079,90\n\
Tesouro IPCA+;15/05/2035;05/01/2026;6,48;6,60;2105,00;2085,30;2084,00\n\
Tesouro Selic;01/03/2029;05/01/2026;0,05;0,06;0,00;16.123,45;16.120,00\n\
Tesouro Prefixado;01/01/2024;28/12/2023;0;0;0,00;0,00;999,10\n";

    #[test]
    fn le_titulos_e_precos() {
        let wanted = |k: &str, _: NaiveDate| k != "Tesouro Selic";
        let data = parse(SAMPLE.as_bytes(), &wanted).unwrap();
        assert_eq!(data.titles.len(), 3);
        let ipca = data
            .titles
            .iter()
            .find(|t| t.kind == "Tesouro IPCA+")
            .unwrap();
        assert_eq!(ipca.last_date, NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
        assert_eq!(data.prices.len(), 3);
        assert_eq!(data.prices[1].price, dec!(2085.30));
        // Sem preço de venda (título vencido): usa o PU base.
        assert_eq!(data.prices[2].price, dec!(999.10));
        // A Selic não foi pedida.
        assert!(data.prices.iter().all(|p| p.kind != "Tesouro Selic"));
    }

    #[test]
    fn cabecalho_inesperado() {
        assert!(parse(b"a;b;c\n1;2;3\n", &|_, _| true).is_err());
    }
}
