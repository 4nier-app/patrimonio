//! "Sobre" em português dos ETFs mais conhecidos, escrito a partir do
//! documento de cada gestora (o índice que o ETF segue, onde o fundo fica e se
//! ele paga ou reinveste os proventos). Não há fonte gratuita com esses textos
//! em português; ativos fora da lista mostram só os dados.
//!
//! Também traduz os setores que o Yahoo informa em inglês.

/// Texto de um ETF conhecido, pelo código como o app guarda (CSPX.L, VOO, IVVB11).
pub fn sobre(ticker: &str) -> Option<&'static str> {
    let t = ticker.trim().to_uppercase();
    TEXTOS.iter().find(|(k, _)| *k == t).map(|(_, v)| *v)
}

macro_rules! texto {
    ($a:expr, $b:expr) => {
        concat!($a, " ", $b)
    };
}

const TEXTOS: &[(&str, &str)] = &[
    // Bolsa de Londres (fundos irlandeses).
    ("CSPX.L", texto!("iShares Core S&P 500, da BlackRock. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VUAA.L", texto!("Vanguard S&P 500, linha que acumula. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VUSD.L", texto!("Vanguard S&P 500, linha que distribui. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA.")),
    ("CNDX.L", texto!("iShares Nasdaq 100, da BlackRock. Segue o Nasdaq-100, as 100 maiores empresas não financeiras da bolsa Nasdaq, com peso grande de tecnologia.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("IWDA.L", texto!("iShares Core MSCI World, da BlackRock. Segue o MSCI World: grandes e médias empresas de países desenvolvidos (EUA, Europa, Japão e outros), sem os emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("SSAC.L", texto!("iShares MSCI ACWI, da BlackRock. Segue o MSCI ACWI: grandes e médias empresas de países desenvolvidos e emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VWRA.L", texto!("Vanguard FTSE All-World, linha que acumula. Segue o FTSE All-World: grandes e médias empresas do mundo todo, desenvolvidos e emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VWRD.L", texto!("Vanguard FTSE All-World, linha que distribui. Segue o FTSE All-World: grandes e médias empresas do mundo todo, desenvolvidos e emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA.")),
    ("VHVE.L", texto!("Vanguard FTSE Developed World, linha que acumula. Segue o FTSE Developed: grandes e médias empresas de países desenvolvidos.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("EIMI.L", texto!("iShares Core MSCI Emerging Markets IMI, da BlackRock. Segue o MSCI Emerging Markets IMI: empresas grandes, médias e pequenas de países emergentes (China, Índia, Taiwan, Brasil e outros).", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("IB01.L", texto!("iShares $ Treasury Bond 0-1yr, da BlackRock. Compra títulos do Tesouro dos EUA que vencem em até 1 ano: rende perto dos juros de curto prazo americanos.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("IGLN.L", "iShares Physical Gold, da BlackRock. Acompanha o preço do ouro: cada cota é lastreada em ouro físico guardado em cofre. É um ETC (título lastreado), emitido na Irlanda e negociado na Bolsa de Londres em dólar. Não paga proventos."),
    ("IB1T.L", "iShares Bitcoin ETP, da BlackRock. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados com um custodiante. É um ETP (título lastreado) emitido na Suíça e negociado na Bolsa de Londres em dólar. Não paga proventos."),
    // EUA.
    ("VOO", texto!("Vanguard S&P 500. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("IVV", texto!("iShares Core S&P 500, da BlackRock. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("SPY", texto!("SPDR S&P 500, da State Street, o ETF mais antigo do S&P 500. Segue as cerca de 500 maiores empresas dos EUA.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VTI", texto!("Vanguard Total Stock Market. Segue o mercado de ações americano inteiro (índice CRSP US Total Market): empresas grandes, médias e pequenas.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VT", texto!("Vanguard Total World Stock. Segue o FTSE Global All Cap: ações do mundo todo, de países desenvolvidos e emergentes.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("QQQ", texto!("Invesco QQQ. Segue o Nasdaq-100, as 100 maiores empresas não financeiras da bolsa Nasdaq, com peso grande de tecnologia.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VNQ", texto!("Vanguard Real Estate. Investe em REITs, os fundos imobiliários dos EUA (escritórios, galpões, torres, residências e outros).", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VIG", texto!("Vanguard Dividend Appreciation. Segue o S&P U.S. Dividend Growers: empresas americanas que aumentam os dividendos há pelo menos 10 anos seguidos.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VYM", texto!("Vanguard High Dividend Yield. Segue o FTSE High Dividend Yield: empresas americanas que pagam dividendos acima da média.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("SCHD", texto!("Schwab U.S. Dividend Equity. Segue o Dow Jones U.S. Dividend 100: 100 empresas americanas com dividendos altos e constantes.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("GLD", "SPDR Gold Shares, da State Street. Acompanha o preço do ouro: as cotas são lastreadas em ouro físico guardado em cofre. É um fundo dos EUA, negociado em dólar, e não paga proventos."),
    ("IAU", "iShares Gold Trust, da BlackRock. Acompanha o preço do ouro: as cotas são lastreadas em ouro físico guardado em cofre. É um fundo dos EUA, negociado em dólar, e não paga proventos."),
    ("IBIT", "iShares Bitcoin Trust, da BlackRock. Acompanha o preço do bitcoin: o fundo guarda bitcoins com um custodiante. É um fundo dos EUA, negociado em dólar, e não paga proventos."),
    ("EWZ", texto!("iShares MSCI Brazil, da BlackRock. Segue o MSCI Brazil 25/50: grandes e médias empresas brasileiras, cotadas em dólar.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("BND", texto!("Vanguard Total Bond Market. Compra títulos de renda fixa dos EUA com grau de investimento: Tesouro americano, empresas e hipotecas.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("TLT", texto!("iShares 20+ Year Treasury Bond, da BlackRock. Compra títulos do Tesouro dos EUA que vencem em mais de 20 anos: oscila bastante com os juros americanos.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    // B3.
    ("IVVB11", "iShares S&P 500 Fundo de Índice, da BlackRock. Investe no ETF americano IVV, que segue o S&P 500, e é negociado em reais na B3: sobe e desce com as empresas americanas e com o dólar. Não paga proventos: reinveste tudo."),
    ("BOVA11", "iShares Ibovespa Fundo de Índice, da BlackRock. Segue o Ibovespa, o principal índice de ações da B3."),
    ("SMAL11", "iShares BM&FBOVESPA Small Cap, da BlackRock. Segue o índice SMLL, das empresas menores da B3."),
    ("DIVO11", "It Now IDIV, do Itaú. Segue o IDIV, índice das empresas da B3 que mais pagam dividendos."),
    ("HASH11", "Hashdex Nasdaq Crypto Index. Segue o Nasdaq Crypto Index, uma cesta das maiores criptomoedas (bitcoin, ethereum e outras), em reais na B3."),
    ("GOLD11", "Trend ETF LBMA Ouro. Acompanha o preço do ouro em reais, negociado na B3."),
];

/// Setor em português, pela chave dos pesos do ETF ("technology") ou pelo
/// nome do setor da ação ("Technology"). `None` para o que não reconhece.
pub fn setor(key: &str) -> Option<&'static str> {
    let k: String = key
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect();
    Some(match k.as_str() {
        "technology" => "Tecnologia",
        "healthcare" => "Saúde",
        "financialservices" | "financial" => "Financeiro",
        "consumercyclical" => "Consumo não essencial",
        "consumerdefensive" => "Consumo essencial",
        "communicationservices" => "Comunicação",
        "industrials" => "Indústria",
        "energy" => "Energia",
        "utilities" => "Energia elétrica e saneamento",
        "realestate" => "Imóveis",
        "basicmaterials" => "Materiais básicos",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn textos_e_setores() {
        assert!(sobre("cspx.l").unwrap().contains("Irlanda"));
        assert!(sobre("VOO").unwrap().contains("30%"));
        assert_eq!(sobre("XXXX"), None);
        assert_eq!(setor("financial_services"), Some("Financeiro"));
        assert_eq!(setor("Consumer Cyclical"), Some("Consumo não essencial"));
        assert_eq!(setor("outro"), None);
        // Cada texto da lista está completo e cita o que o fundo faz.
        for (k, v) in TEXTOS {
            assert!(v.len() > 60, "{k}");
            assert!(v.ends_with('.'), "{k}");
        }
    }

    #[test]
    fn londres_coincide_com_o_catalogo() {
        for e in crate::london::ETFS {
            assert!(sobre(e.ticker).is_some(), "falta o texto de {}", e.ticker);
        }
    }
}
