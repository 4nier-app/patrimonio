//! Extrato de Negociação da Área do Investidor da B3 (planilha Excel): as
//! compras e vendas de todas as corretoras. Não traz as taxas, que o
//! aplicativo calcula pelas regras da B3, como num lançamento manual.

use std::str::FromStr;

use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Serialize;

use crate::{xlsx, ProviderError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TradeSide {
    Compra,
    Venda,
}

/// Uma linha do extrato.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct B3Trade {
    /// Linha na planilha (a 1 é o cabeçalho).
    pub line: usize,
    pub date: NaiveDate,
    pub side: TradeSide,
    /// "Mercado à Vista" ou "Mercado Fracionário".
    pub market: String,
    /// Corretora, como a B3 escreve ("XP INVESTIMENTOS CCTVM S/A").
    pub institution: String,
    /// Código como veio (o fracionário termina em F).
    pub ticker: String,
    pub quantity: Decimal,
    pub price: Decimal,
    pub value: Decimal,
}

/// Linha que o aplicativo não importa, com o motivo.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Skipped {
    pub line: usize,
    pub text: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct B3Extract {
    pub trades: Vec<B3Trade>,
    pub skipped: Vec<Skipped>,
}

const HEADER: [&str; 9] = [
    "data do negócio",
    "tipo de movimentação",
    "mercado",
    "prazo/vencimento",
    "instituição",
    "código de negociação",
    "quantidade",
    "preço",
    "valor",
];

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// Número como gravado no Excel ("12.48", "12.480000000000001", "1E-3"). O
/// Excel guarda em ponto flutuante: arredonda em 8 casas, que cobre os preços.
fn number(s: &str) -> Option<Decimal> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let d = Decimal::from_str(t)
        .or_else(|_| Decimal::from_scientific(t))
        .ok()?;
    Some(d.round_dp(8).normalize())
}

fn date(s: &str) -> Option<NaiveDate> {
    let t = s.trim();
    NaiveDate::parse_from_str(t, "%d/%m/%Y").ok().or_else(|| {
        // Data como número de série do Excel (dias desde 30/12/1899).
        let n: i64 = t.split('.').next()?.parse().ok()?;
        NaiveDate::from_ymd_opt(1899, 12, 30)?.checked_add_signed(chrono::Duration::days(n))
    })
}

/// Lê o extrato de negociação. Erro se a planilha não for esse extrato.
pub fn read_trades(bytes: &[u8]) -> Result<B3Extract> {
    let rows = xlsx::first_sheet(bytes)?;
    let header: Vec<String> = rows
        .first()
        .map(|r| r.iter().map(|c| norm(c)).collect())
        .unwrap_or_default();
    if header.first().map(String::as_str) == Some("entrada/saída") {
        return Err(ProviderError::Format(
            "este é o extrato de movimentação; para importar as compras e vendas, use o extrato de negociação".into(),
        ));
    }
    let col = |name: &str| header.iter().position(|h| h == name);
    let cols: Vec<usize> = HEADER
        .iter()
        .map(|h| {
            col(h).ok_or_else(|| {
                ProviderError::Format(
                    "a planilha não é o extrato de negociação da B3 (faltam colunas)".into(),
                )
            })
        })
        .collect::<Result<_>>()?;
    let mut trades = Vec::new();
    let mut skipped = Vec::new();
    for (i, row) in rows.iter().enumerate().skip(1) {
        let line = i + 1;
        let get = |k: usize| row.get(cols[k]).map(String::as_str).unwrap_or("").trim();
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let text = row
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let skip = |reason: &str| Skipped {
            line,
            text: text.clone(),
            reason: reason.into(),
        };
        let side = match norm(get(1)).as_str() {
            "compra" => TradeSide::Compra,
            "venda" => TradeSide::Venda,
            _ => {
                skipped.push(skip("tipo de movimentação que o aplicativo não importa"));
                continue;
            }
        };
        let market = get(2).to_string();
        let m = norm(&market);
        if !(m.contains("vista") || m.contains("fracion")) {
            skipped.push(skip(
                "mercado que o aplicativo ainda não acompanha (opções, termo ou futuro)",
            ));
            continue;
        }
        let (Some(date), Some(quantity), Some(price)) =
            (date(get(0)), number(get(6)), number(get(7)))
        else {
            skipped.push(skip("data, quantidade ou preço ilegível"));
            continue;
        };
        if quantity <= Decimal::ZERO || price <= Decimal::ZERO {
            skipped.push(skip("quantidade ou preço zerado"));
            continue;
        }
        let ticker = get(5).to_uppercase();
        if ticker.is_empty() {
            skipped.push(skip("sem o código do ativo"));
            continue;
        }
        trades.push(B3Trade {
            line,
            date,
            side,
            market,
            institution: get(4).to_string(),
            ticker,
            quantity,
            price,
            value: number(get(8))
                .map(|v| v.round_dp(2))
                .unwrap_or_else(|| (quantity * price).round_dp(2)),
        });
    }
    Ok(B3Extract { trades, skipped })
}

/// O que uma linha do extrato de movimentação representa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MovementKind {
    Dividendo,
    /// O valor no extrato já vem com os 15% de imposto descontados.
    Jcp,
    Rendimento,
    Bonificacao,
    Desdobramento,
}

/// Uma linha do extrato de movimentação que o aplicativo sabe lançar.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct B3Movement {
    pub line: usize,
    pub date: NaiveDate,
    pub kind: MovementKind,
    /// Como a B3 escreve ("Juros Sobre Capital Próprio").
    pub description: String,
    pub ticker: String,
    pub institution: String,
    pub quantity: Option<Decimal>,
    pub unit_price: Option<Decimal>,
    pub value: Option<Decimal>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct B3Movements {
    pub movements: Vec<B3Movement>,
    pub skipped: Vec<Skipped>,
}

const MOV_HEADER: [&str; 8] = [
    "entrada/saída",
    "data",
    "movimentação",
    "produto",
    "instituição",
    "quantidade",
    "preço unitário",
    "valor da operação",
];

/// "-" quer dizer vazio no extrato de movimentação.
fn opt_number(s: &str) -> Option<Decimal> {
    if s.trim() == "-" {
        None
    } else {
        number(s)
    }
}

/// Lê o extrato de movimentação: proventos, bonificações e desdobramentos.
/// O resto (liquidações, transferências, frações, leilões...) fica de fora,
/// com o motivo.
pub fn read_movements(bytes: &[u8]) -> Result<B3Movements> {
    let rows = xlsx::first_sheet(bytes)?;
    let header: Vec<String> = rows
        .first()
        .map(|r| r.iter().map(|c| norm(c)).collect())
        .unwrap_or_default();
    let cols: Vec<usize> = MOV_HEADER
        .iter()
        .map(|h| {
            header.iter().position(|c| c == h).ok_or_else(|| {
                ProviderError::Format(
                    "a planilha não é o extrato de movimentação da B3 (faltam colunas)".into(),
                )
            })
        })
        .collect::<Result<_>>()?;
    let mut movements = Vec::new();
    let mut skipped = Vec::new();
    for (i, row) in rows.iter().enumerate().skip(1) {
        let line = i + 1;
        let get = |k: usize| row.get(cols[k]).map(String::as_str).unwrap_or("").trim();
        if row.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        let text = row
            .iter()
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .collect::<Vec<_>>()
            .join(" · ");
        let skip = |reason: &str| Skipped {
            line,
            text: text.clone(),
            reason: reason.into(),
        };
        let description = get(2).to_string();
        let d = norm(&description);
        let kind = if d.contains("transferido") {
            None
        } else if d == "dividendo" {
            Some(MovementKind::Dividendo)
        } else if d.starts_with("juros sobre capital") {
            Some(MovementKind::Jcp)
        } else if d == "rendimento" {
            Some(MovementKind::Rendimento)
        } else if d.starts_with("bonificação") {
            Some(MovementKind::Bonificacao)
        } else if d.starts_with("desdobro") || d.starts_with("desdobramento") {
            Some(MovementKind::Desdobramento)
        } else {
            None
        };
        let Some(kind) = kind else {
            let reason = if d.contains("liquidação") {
                "compra ou venda: use o extrato de negociação"
            } else if d.contains("transferido") {
                "repete outro lançamento do extrato"
            } else if d.contains("grupamento") {
                "grupamento: lance em Eventos, informando a proporção"
            } else {
                "movimentação que o aplicativo não importa"
            };
            skipped.push(skip(reason));
            continue;
        };
        if norm(get(0)) != "credito" && norm(get(0)) != "crédito" {
            skipped.push(skip("saída da conta, não é um provento recebido"));
            continue;
        }
        let Some(date) = date(get(1)) else {
            skipped.push(skip("data ilegível"));
            continue;
        };
        let ticker = get(3)
            .split(" - ")
            .next()
            .unwrap_or("")
            .trim()
            .to_uppercase();
        if ticker.is_empty() {
            skipped.push(skip("sem o código do ativo"));
            continue;
        }
        let value = opt_number(get(7)).map(|v| v.round_dp(2));
        let quantity = opt_number(get(5));
        let is_income = matches!(
            kind,
            MovementKind::Dividendo | MovementKind::Jcp | MovementKind::Rendimento
        );
        if is_income && !value.is_some_and(|v| v > Decimal::ZERO) {
            skipped.push(skip("provento sem valor"));
            continue;
        }
        if !is_income && !quantity.is_some_and(|q| q > Decimal::ZERO) {
            skipped.push(skip("evento sem a quantidade recebida"));
            continue;
        }
        movements.push(B3Movement {
            line,
            date,
            kind,
            description,
            ticker,
            institution: get(4).to_string(),
            quantity,
            unit_price: opt_number(get(6)),
            value,
        });
    }
    Ok(B3Movements { movements, skipped })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xlsx::tests::build;
    use rust_decimal_macros::dec;

    const H: [&str; 9] = [
        "Data do Negócio",
        "Tipo de Movimentação",
        "Mercado",
        "Prazo/Vencimento",
        "Instituição",
        "Código de Negociação",
        "Quantidade",
        "Preço",
        "Valor",
    ];

    #[test]
    fn le_compras_vendas_e_pula_o_que_nao_importa() {
        // Dados inventados.
        let bytes = build(&[
            H.to_vec(),
            vec![
                "05/03/2026",
                "Compra",
                "Mercado à Vista",
                "-",
                "CORRETORA EXEMPLO S/A",
                "ABCD3",
                "100",
                "10.120000000000001",
                "1012",
            ],
            vec![
                "05/03/2026",
                "Compra",
                "Mercado Fracionário",
                "-",
                "CORRETORA EXEMPLO S/A",
                "ABCD3F",
                "7",
                "10.13",
                "70.91",
            ],
            vec![
                "06/03/2026",
                "Venda",
                "Mercado à Vista",
                "-",
                "OUTRA CORRETORA",
                "WXYZ11",
                "10",
                "98.5",
                "985",
            ],
            vec![
                "06/03/2026",
                "Compra",
                "Opção de Compra",
                "20/03/2026",
                "OUTRA CORRETORA",
                "ABCDC120",
                "100",
                "0.35",
                "35",
            ],
            vec!["", "", "", "", "", "", "", "", ""],
        ]);
        let x = read_trades(&bytes).unwrap();
        assert_eq!(x.trades.len(), 3);
        let a = &x.trades[0];
        assert_eq!(
            (a.line, a.side, a.quantity, a.price),
            (2, TradeSide::Compra, dec!(100), dec!(10.12))
        );
        assert_eq!(a.date, NaiveDate::from_ymd_opt(2026, 3, 5).unwrap());
        assert_eq!(x.trades[1].ticker, "ABCD3F");
        assert_eq!(x.trades[2].side, TradeSide::Venda);
        assert_eq!(x.skipped.len(), 1);
        assert!(x.skipped[0].reason.contains("opções"));
    }

    #[test]
    fn recusa_outra_planilha() {
        let mov = build(&[vec!["Entrada/Saída", "Data", "Movimentação"]]);
        assert!(read_trades(&mov)
            .unwrap_err()
            .to_string()
            .contains("movimentação"));
        let other = build(&[vec!["Nome", "Valor"]]);
        assert!(read_trades(&other).is_err());
    }
    #[test]
    fn le_proventos_e_eventos_da_movimentacao() {
        // Dados inventados.
        let h = vec![
            "Entrada/Saída",
            "Data",
            "Movimentação",
            "Produto",
            "Instituição",
            "Quantidade",
            "Preço unitário",
            "Valor da Operação",
        ];
        let c = "CORRETORA EXEMPLO S/A";
        let bytes = build(&[
            h,
            vec!["Credito", "15/04/2026", "Dividendo", "ABCD3 - ABCD SA", c, "100", "0.5", "50"],
            vec!["Credito", "15/04/2026", "Juros Sobre Capital Próprio", "ABCD3 - ABCD SA", c, "100", "0.85", "85"],
            vec!["Credito", "10/04/2026", "Rendimento", "WXYZ11 - FII WXYZ", c, "10", "1", "10"],
            vec!["Credito", "10/04/2026", "Rendimento - Transferido", "WXYZ11 - FII WXYZ", c, "10", "1", "10"],
            vec!["Credito", "20/04/2026", "Bonificação em Ativos", "ABCD3 - ABCD SA", c, "10", "5", "-"],
            vec!["Credito", "21/04/2026", "Desdobro", "EFGH3 - EFGH SA", c, "100", "-", "-"],
            vec!["Credito", "22/04/2026", "Transferência - Liquidação", "ABCD3 - ABCD SA", c, "100", "10", "1000"],
            vec!["Debito", "23/04/2026", "Dividendo", "ABCD3 - ABCD SA", c, "100", "0.5", "50"],
            vec!["Credito", "24/04/2026", "Fração em Ativos", "ABCD3 - ABCD SA", c, "0.4", "-", "-"],
        ]);
        let x = read_movements(&bytes).unwrap();
        let kinds: Vec<_> = x.movements.iter().map(|m| m.kind).collect();
        assert_eq!(
            kinds,
            vec![
                MovementKind::Dividendo,
                MovementKind::Jcp,
                MovementKind::Rendimento,
                MovementKind::Bonificacao,
                MovementKind::Desdobramento
            ]
        );
        assert_eq!(x.movements[0].ticker, "ABCD3");
        assert_eq!(x.movements[1].value, Some(dec!(85)));
        assert_eq!(x.movements[3].value, None);
        assert_eq!(x.movements[3].unit_price, Some(dec!(5)));
        assert_eq!(x.movements[4].quantity, Some(dec!(100)));
        assert_eq!(x.skipped.len(), 4);
        assert!(x.skipped[1].reason.contains("negociação"));
        assert!(read_movements(&build(&[vec!["Nome", "Valor"]])).is_err());
    }
}
