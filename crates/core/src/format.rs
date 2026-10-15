//! Formatação brasileira de números para a memória de cálculo.

use rust_decimal::{Decimal, RoundingStrategy};

fn group(digits: &str) -> String {
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(c);
    }
    out
}

/// Número com `dp` casas, separador de milhar "." e decimal ",".
pub fn number(v: Decimal, dp: u32) -> String {
    let r = v.round_dp_with_strategy(dp, RoundingStrategy::MidpointAwayFromZero);
    let s = format!("{:.*}", dp as usize, r);
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-');
    let (int, frac) = s
        .split_once('.')
        .map(|(a, b)| (a, Some(b)))
        .unwrap_or((s, None));
    let mut out = String::new();
    if neg && !r.is_zero() {
        out.push('-');
    }
    out.push_str(&group(int));
    if let Some(f) = frac {
        out.push(',');
        out.push_str(f);
    }
    out
}

/// Valor em reais: "R$ 1.234,56".
pub fn brl(v: Decimal) -> String {
    format!("R$ {}", number(v, 2))
}

/// Fração como percentual: 0,0523 → "5,23%".
pub fn pct(fraction: Decimal, dp: u32) -> String {
    format!("{}%", number(fraction * Decimal::ONE_HUNDRED, dp))
}

/// Quantidade sem zeros à direita: 100 → "100"; 0,5000 → "0,5".
pub fn qty(v: Decimal) -> String {
    let n = v.normalize();
    let dp = n.scale();
    number(n, dp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn formatos() {
        assert_eq!(brl(dec!(1234567.891)), "R$ 1.234.567,89");
        assert_eq!(brl(dec!(-0.004)), "R$ 0,00");
        assert_eq!(brl(dec!(-12.5)), "R$ -12,50");
        assert_eq!(pct(dec!(0.052317), 2), "5,23%");
        assert_eq!(qty(dec!(100.000)), "100");
        assert_eq!(qty(dec!(0.12340)), "0,1234");
    }
}
