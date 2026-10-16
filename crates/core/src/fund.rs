//! Fundos de investimento: identificados pelo CNPJ, lançados à mão pelo
//! extrato do banco (aplicação e resgate em cotas, com o valor da cota do
//! dia) e cotados pelo informe diário que os fundos entregam à CVM.
//!
//! O código do fundo na carteira é o próprio CNPJ formatado
//! ("12.345.678/0001-95"), para que o fundo seja reconhecido sem consultar o
//! catálogo, como as criptomoedas. Não há taxas da B3: o que o banco desconta
//! no resgate (IR e IOF) é informado no lançamento, como no extrato.

pub const KIND: &str = "FUNDO";

/// Só os 14 dígitos, se o texto for um CNPJ (com ou sem pontuação).
pub fn cnpj_digits(text: &str) -> Option<String> {
    let t = text.trim();
    if !t
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | '/' | '-' | ' '))
    {
        return None;
    }
    let digits: String = t.chars().filter(char::is_ascii_digit).collect();
    (digits.len() == 14).then_some(digits)
}

/// Confere os dois dígitos verificadores.
pub fn valid_cnpj(digits: &str) -> bool {
    let d: Vec<u32> = digits.chars().filter_map(|c| c.to_digit(10)).collect();
    if d.len() != 14 || d.iter().all(|&x| x == d[0]) {
        return false;
    }
    let check = |n: usize| {
        let weights: Vec<u32> = (2..=9).cycle().take(n).collect();
        let sum: u32 = d[..n].iter().rev().zip(&weights).map(|(a, w)| a * w).sum();
        match sum % 11 {
            0 | 1 => 0,
            r => 11 - r,
        }
    };
    check(12) == d[12] && check(13) == d[13]
}

/// "12345678000195" → "12.345.678/0001-95".
pub fn format_cnpj(digits: &str) -> String {
    let d = digits;
    format!(
        "{}.{}.{}/{}-{}",
        &d[0..2],
        &d[2..5],
        &d[5..8],
        &d[8..12],
        &d[12..14]
    )
}

/// Código do fundo na carteira, se o texto for um CNPJ válido.
pub fn ticker_of(text: &str) -> Option<String> {
    cnpj_digits(text)
        .filter(|d| valid_cnpj(d))
        .map(|d| format_cnpj(&d))
}

/// O código é de um fundo (CNPJ formatado)?
pub fn is_fund(ticker: &str) -> bool {
    let b = ticker.as_bytes();
    b.len() == 18
        && b[2] == b'.'
        && b[6] == b'.'
        && b[10] == b'/'
        && b[15] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| matches!(i, 2 | 6 | 10 | 15) || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    // CNPJs fictícios, só com dígitos verificadores corretos.
    #[test]
    fn reconhece_e_formata_o_cnpj() {
        assert_eq!(
            ticker_of("11222333000181").as_deref(),
            Some("11.222.333/0001-81")
        );
        assert_eq!(
            ticker_of(" 11.222.333/0001-81 ").as_deref(),
            Some("11.222.333/0001-81")
        );
        assert_eq!(ticker_of("11222333000182"), None, "dígito errado");
        assert_eq!(ticker_of("11111111111111"), None);
        assert_eq!(ticker_of("ABCD3"), None);
        assert!(is_fund("11.222.333/0001-81"));
        assert!(!is_fund("ABCD3"));
        assert!(!is_fund("11222333000181"));
    }
}
