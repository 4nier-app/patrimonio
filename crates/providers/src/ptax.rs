//! PTAX do dólar pelo serviço de dados abertos do Banco Central (Olinda).
//!
//! O rendimento no exterior é convertido pela cotação de venda do boletim de
//! fechamento (Lei 14.754/2023, art. 15). O serviço devolve todos os
//! boletins do dia; o de fechamento é o último.

use std::collections::BTreeMap;
use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{http, ProviderError, Result};

pub const SOURCE: &str = "BCB-PTAX";

#[derive(Debug, Clone, PartialEq)]
pub struct Ptax {
    pub date: NaiveDate,
    pub buy: Decimal,
    pub sell: Decimal,
}

pub fn url(from: NaiveDate, to: NaiveDate) -> String {
    format!(
        "https://olinda.bcb.gov.br/olinda/servico/PTAX/versao/v1/odata/\
         CotacaoDolarPeriodo(dataInicial=@dataInicial,dataFinalCotacao=@dataFinalCotacao)\
         ?@dataInicial=%27{}%27&@dataFinalCotacao=%27{}%27&$top=10000&$format=json\
         &$select=cotacaoCompra,cotacaoVenda,dataHoraCotacao",
        from.format("%m-%d-%Y"),
        to.format("%m-%d-%Y")
    )
}

#[derive(Deserialize)]
struct Envelope {
    value: Vec<Row>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Row {
    cotacao_compra: f64,
    cotacao_venda: f64,
    data_hora_cotacao: String,
}

/// A PTAX é publicada com 4 casas decimais.
fn four(v: f64) -> Result<Decimal> {
    Decimal::from_str(&format!("{v:.4}"))
        .map_err(|e| ProviderError::Format(format!("cotação inválida {v}: {e}")))
}

/// Interpreta a resposta: um boletim por dia, o último publicado no dia.
pub fn parse(json: &[u8]) -> Result<Vec<Ptax>> {
    let env: Envelope =
        serde_json::from_slice(json).map_err(|e| ProviderError::Format(e.to_string()))?;
    let mut by_day: BTreeMap<NaiveDate, (String, Ptax)> = BTreeMap::new();
    for row in env.value {
        let day = row
            .data_hora_cotacao
            .get(..10)
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .ok_or_else(|| {
                ProviderError::Format(format!("data inválida {}", row.data_hora_cotacao))
            })?;
        let ptax = Ptax {
            date: day,
            buy: four(row.cotacao_compra)?,
            sell: four(row.cotacao_venda)?,
        };
        match by_day.get(&day) {
            Some((stamp, _)) if *stamp >= row.data_hora_cotacao => {}
            _ => {
                by_day.insert(day, (row.data_hora_cotacao, ptax));
            }
        }
    }
    Ok(by_day.into_values().map(|(_, p)| p).collect())
}

pub fn fetch(from: NaiveDate, to: NaiveDate) -> Result<Vec<Ptax>> {
    parse(&http::get_bytes(&url(from, to))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn um_boletim_por_dia_o_de_fechamento() {
        // Resposta sintética no formato do serviço.
        let json = br#"{"@odata.context":"x","value":[
            {"cotacaoCompra":5.1001,"cotacaoVenda":5.1007,"dataHoraCotacao":"2026-03-02 13:05:12.345"},
            {"cotacaoCompra":5.2,"cotacaoVenda":5.2006,"dataHoraCotacao":"2026-03-03 13:04:00.1"},
            {"cotacaoCompra":5.1999,"cotacaoVenda":5.2005,"dataHoraCotacao":"2026-03-03 11:04:00.1"}
        ]}"#;
        let p = parse(json).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].sell, dec!(5.1007));
        assert_eq!(p[1].date, NaiveDate::from_ymd_opt(2026, 3, 3).unwrap());
        assert_eq!(p[1].sell, dec!(5.2006));
        assert_eq!(p[1].buy, dec!(5.2000));
    }

    #[test]
    fn url_com_datas_no_formato_do_bcb() {
        let u = url(
            NaiveDate::from_ymd_opt(2025, 1, 2).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        assert!(u.contains("@dataInicial=%2701-02-2025%27"));
        assert!(u.contains("@dataFinalCotacao=%2712-31-2025%27"));
    }
}
