//! "Sobre" em português dos ETFs mais conhecidos, escrito a partir do
//! documento de cada gestora (o índice que o ETF segue, onde o fundo fica e se
//! ele paga ou reinveste os proventos). Não há fonte gratuita com esses textos
//! em português; ativos fora da lista mostram só os dados.
//!
//! Também traduz os setores que o Yahoo informa em inglês.

/// Texto de um ETF conhecido e a data em que foi escrito ou revisado
/// (AAAA-MM-DD). O texto não é atualizado pela internet: a tela mostra a data
/// para o usuário saber que pode ter mudado depois dela.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sobre {
    pub texto: &'static str,
    pub revisado: &'static str,
}

/// Texto de um ETF conhecido, pelo código como o app guarda (CSPX.L, VOO, IVVB11).
pub fn sobre(ticker: &str) -> Option<Sobre> {
    let t = ticker.trim().to_uppercase();
    TEXTOS
        .iter()
        .find(|(k, _, _)| *k == t)
        .map(|(_, revisado, texto)| Sobre { texto, revisado })
}

macro_rules! texto {
    ($a:expr, $b:expr) => {
        concat!($a, " ", $b)
    };
}

/// Código, data em que o texto foi escrito ou revisado e o texto.
const TEXTOS: &[(&str, &str, &str)] = &[
    // Bolsa de Londres (fundos irlandeses).
    ("CSPX.L", "2026-11-12", texto!("iShares Core S&P 500, da BlackRock. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VUAA.L", "2026-11-12", texto!("Vanguard S&P 500, linha que acumula. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VUSD.L", "2026-11-12", texto!("Vanguard S&P 500, linha que distribui. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA.")),
    ("CNDX.L", "2026-11-12", texto!("iShares Nasdaq 100, da BlackRock. Segue o Nasdaq-100, as 100 maiores empresas não financeiras da bolsa Nasdaq, com peso grande de tecnologia.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("IWDA.L", "2026-11-12", texto!("iShares Core MSCI World, da BlackRock. Segue o MSCI World: grandes e médias empresas de países desenvolvidos (EUA, Europa, Japão e outros), sem os emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("ISAC.L", "2026-11-12", texto!("iShares MSCI ACWI, da BlackRock. Segue o MSCI ACWI: grandes e médias empresas de países desenvolvidos e emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VWRA.L", "2026-11-12", texto!("Vanguard FTSE All-World, linha que acumula. Segue o FTSE All-World: grandes e médias empresas do mundo todo, desenvolvidos e emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("VWRD.L", "2026-11-12", texto!("Vanguard FTSE All-World, linha que distribui. Segue o FTSE All-World: grandes e médias empresas do mundo todo, desenvolvidos e emergentes.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA.")),
    ("VHVE.L", "2026-11-12", texto!("Vanguard FTSE Developed World, linha que acumula. Segue o FTSE Developed: grandes e médias empresas de países desenvolvidos.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("EIMI.L", "2026-11-12", texto!("iShares Core MSCI Emerging Markets IMI, da BlackRock. Segue o MSCI Emerging Markets IMI: empresas grandes, médias e pequenas de países emergentes (China, Índia, Taiwan, Brasil e outros).", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("IB01.L", "2026-11-12", texto!("iShares $ Treasury Bond 0-1yr, da BlackRock. Compra títulos do Tesouro dos EUA que vencem em até 1 ano: rende perto dos juros de curto prazo americanos.", "É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo.")),
    ("IGLN.L", "2026-11-12", "iShares Physical Gold, da BlackRock. Acompanha o preço do ouro: cada cota é lastreada em ouro físico guardado em cofre. É um ETC (título lastreado), emitido na Irlanda e negociado na Bolsa de Londres em dólar. Não paga proventos."),
    ("SPYL.L", "2026-11-13", "SPDR S&P 500, da State Street, linha que acumula. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("SPY5.L", "2026-11-13", "SPDR S&P 500, da State Street, linha que distribui. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("SPXS.L", "2026-11-13", "Invesco S&P 500. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA. Não compra as ações: recebe o retorno do índice por contratos com bancos (réplica sintética), por isso dentro do fundo não há o imposto de 15% sobre os dividendos americanos; em troca, depende desses bancos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("EQQU.L", "2026-11-13", "Invesco EQQQ Nasdaq-100. Segue o Nasdaq-100, as 100 maiores empresas não financeiras da bolsa Nasdaq, com peso grande de tecnologia. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("SWRD.L", "2026-11-13", "SPDR MSCI World, da State Street. Segue o MSCI World: grandes e médias empresas de países desenvolvidos, sem os emergentes. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("XDWD.L", "2026-11-13", "Xtrackers MSCI World, da DWS. Segue o MSCI World: grandes e médias empresas de países desenvolvidos, sem os emergentes. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("FWRA.L", "2026-11-13", "Invesco FTSE All-World. Segue o FTSE All-World: grandes e médias empresas do mundo todo, desenvolvidos e emergentes. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDEV.L", "2026-11-13", "Vanguard FTSE Developed World, linha que distribui. Segue o FTSE Developed: grandes e médias empresas de países desenvolvidos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("VFEA.L", "2026-11-13", "Vanguard FTSE Emerging Markets, linha que acumula. Segue o FTSE Emerging: grandes e médias empresas de países emergentes (China, Índia, Taiwan, Brasil e outros). É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDEM.L", "2026-11-13", "Vanguard FTSE Emerging Markets, linha que distribui. Segue o FTSE Emerging: grandes e médias empresas de países emergentes (China, Índia, Taiwan, Brasil e outros). É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("IEMA.L", "2026-11-13", "iShares MSCI EM, da BlackRock. Segue o MSCI Emerging Markets: grandes e médias empresas de países emergentes. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IBTU.L", "2026-11-13", "iShares $ Treasury Bond 0-1yr, da BlackRock, linha que distribui. Compra títulos do Tesouro dos EUA que vencem em até 1 ano: rende perto dos juros curtos americanos e oscila pouco. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("IBTA.L", "2026-11-13", "iShares $ Treasury Bond 1-3yr, da BlackRock. Compra títulos do Tesouro dos EUA que vencem entre 1 e 3 anos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("CBU0.L", "2026-11-13", "iShares $ Treasury Bond 7-10yr, da BlackRock. Compra títulos do Tesouro dos EUA que vencem entre 7 e 10 anos: oscila mais com a mudança dos juros. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IDTL.L", "2026-11-13", "iShares $ Treasury Bond 20+yr, da BlackRock, linha que distribui. Compra títulos do Tesouro dos EUA que vencem em mais de 20 anos: oscila muito quando os juros americanos mudam. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("DTLA.L", "2026-11-13", "iShares $ Treasury Bond 20+yr, da BlackRock, linha que acumula. Compra títulos do Tesouro dos EUA que vencem em mais de 20 anos: oscila muito quando os juros americanos mudam. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDTA.L", "2026-11-13", "Vanguard USD Treasury Bond, linha que acumula. Compra títulos do Tesouro dos EUA de vários prazos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDTY.L", "2026-11-13", "Vanguard USD Treasury Bond, linha que distribui. Compra títulos do Tesouro dos EUA de vários prazos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("IGLA.L", "2026-11-13", "iShares Global Govt Bond, da BlackRock. Compra títulos de governos de países desenvolvidos, de vários prazos e moedas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IGLO.L", "2026-11-13", "iShares Global Govt Bond, da BlackRock, linha que distribui. Compra títulos de governos de países desenvolvidos, de vários prazos e moedas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("IUAA.L", "2026-11-13", "iShares US Aggregate Bond, da BlackRock. Compra títulos de renda fixa dos EUA com grau de investimento: Tesouro, empresas e hipotecas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("AGGU.L", "2026-11-13", "iShares Global Aggregate Bond, da BlackRock, com proteção cambial para o dólar. Compra títulos de renda fixa com grau de investimento do mundo todo, de governos e empresas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VAGU.L", "2026-11-13", "Vanguard Global Aggregate Bond, com proteção cambial para o dólar. Compra títulos de renda fixa com grau de investimento do mundo todo, de governos e empresas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("LQDA.L", "2026-11-13", "iShares $ Corp Bond, da BlackRock. Compra títulos de empresas com grau de investimento, em dólar. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDCP.L", "2026-11-13", "Vanguard USD Corporate Bond, linha que distribui. Compra títulos de empresas com grau de investimento, em dólar. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("VDUC.L", "2026-11-13", "Vanguard USD Corporate 1-3 Year Bond, linha que distribui. Compra títulos de empresas com grau de investimento, em dólar, que vencem entre 1 e 3 anos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("IHYA.L", "2026-11-13", "iShares $ High Yield Corp Bond, da BlackRock. Compra títulos de empresas sem grau de investimento (alto rendimento e mais risco de calote), em dólar. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("JPST.L", "2026-11-13", "JPMorgan USD Ultra-Short Income, linha que distribui. Fundo ativo que compra títulos de curtíssimo prazo em dólar, de empresas e governos: rende perto dos juros curtos americanos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("PHAU.L", "2026-11-13", "WisdomTree Physical Gold. Acompanha o preço do ouro: cada cota é lastreada em ouro físico guardado em cofre. É um título lastreado (ETC) emitido por uma empresa de Jersey, negociado na Bolsa de Londres em dólar. Não paga proventos."),
    ("SGLD.L", "2026-11-13", "Invesco Physical Gold. Acompanha o preço do ouro: cada cota é lastreada em ouro físico guardado em cofre. É um título lastreado (ETC) emitido por uma empresa da Irlanda, negociado na Bolsa de Londres em dólar. Não paga proventos."),
    ("PHAG.L", "2026-11-13", "WisdomTree Physical Silver. Acompanha o preço da prata: cada cota é lastreada em prata física guardada em cofre. É um título lastreado (ETC) emitido por uma empresa de Jersey, negociado na Bolsa de Londres em dólar. Não paga proventos."),
    ("PHPT.L", "2026-11-13", "WisdomTree Physical Platinum. Acompanha o preço da platina: cada cota é lastreada em platina física guardada em cofre. É um título lastreado (ETC) emitido por uma empresa de Jersey, negociado na Bolsa de Londres em dólar. Não paga proventos."),
    ("BCOM.L", "2026-11-13", "L&G All Commodities. Segue um índice de contratos futuros de várias mercadorias: energia, metais e agrícolas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("IUIT.L", "2026-11-13", "iShares S&P 500 Information Technology Sector, da BlackRock. Compra as empresas de tecnologia do S&P 500. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("XUTC.L", "2026-11-13", "Xtrackers MSCI USA Information Technology, da DWS. Compra as empresas de tecnologia dos EUA. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("IUHC.L", "2026-11-13", "iShares S&P 500 Health Care Sector, da BlackRock. Compra as empresas de saúde do S&P 500. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IUFS.L", "2026-11-13", "iShares S&P 500 Financials Sector, da BlackRock. Compra as empresas financeiras do S&P 500 (bancos, seguradoras, gestoras). É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IUES.L", "2026-11-13", "iShares S&P 500 Energy Sector, da BlackRock. Compra as empresas de energia do S&P 500 (petróleo e gás). É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("WSML.L", "2026-11-13", "iShares MSCI World Small Cap, da BlackRock. Segue o MSCI World Small Cap: empresas menores de países desenvolvidos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IMID.L", "2026-11-13", "SPDR MSCI ACWI IMI, da State Street. Segue o MSCI ACWI IMI: empresas grandes, médias e pequenas do mundo todo, desenvolvidos e emergentes. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("VNRA.L", "2026-11-13", "Vanguard FTSE North America, linha que acumula. Segue o FTSE North America: grandes e médias empresas dos EUA e do Canadá. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDNR.L", "2026-11-13", "Vanguard FTSE North America, linha que distribui. Segue o FTSE North America: grandes e médias empresas dos EUA e do Canadá. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("VJPA.L", "2026-11-13", "Vanguard FTSE Japan, linha que acumula. Segue o FTSE Japan: grandes e médias empresas do Japão. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VDJP.L", "2026-11-13", "Vanguard FTSE Japan, linha que distribui. Segue o FTSE Japan: grandes e médias empresas do Japão. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("VDPX.L", "2026-11-13", "Vanguard FTSE Developed Asia Pacific ex Japan, linha que distribui. Segue grandes e médias empresas de Austrália, Coreia do Sul, Hong Kong, Singapura e Nova Zelândia. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("XDEW.L", "2026-11-13", "Xtrackers S&P 500 Equal Weight, da DWS. Compra as empresas do S&P 500 com o mesmo peso cada uma, em vez de dar mais peso às maiores. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("USSC.L", "2026-11-13", "SPDR MSCI USA Small Cap Value Weighted, da State Street. Compra empresas menores dos EUA, com mais peso nas mais baratas pelo lucro e pelo patrimônio. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("IWQU.L", "2026-11-13", "iShares Edge MSCI World Quality Factor, da BlackRock. Compra empresas de países desenvolvidos com lucro alto e estável e pouca dívida. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IWMO.L", "2026-11-13", "iShares Edge MSCI World Momentum Factor, da BlackRock. Compra empresas de países desenvolvidos cujas ações mais subiram nos últimos meses. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IWVL.L", "2026-11-13", "iShares Edge MSCI World Value Factor, da BlackRock. Compra empresas de países desenvolvidos que estão baratas pelo lucro e pelo patrimônio. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("MVOL.L", "2026-11-13", "iShares Edge MSCI World Minimum Volatility, da BlackRock. Compra ações de países desenvolvidos escolhidas para oscilar menos que o mercado. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("IUQA.L", "2026-11-13", "iShares Edge MSCI USA Quality Factor, da BlackRock. Compra empresas dos EUA com lucro alto e estável e pouca dívida. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("IUMO.L", "2026-11-13", "iShares Edge MSCI USA Momentum Factor, da BlackRock. Compra empresas dos EUA cujas ações mais subiram nos últimos meses. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("IUVL.L", "2026-11-13", "iShares Edge MSCI USA Value Factor, da BlackRock. Compra empresas dos EUA que estão baratas pelo lucro e pelo patrimônio. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("IDUP.L", "2026-11-13", "iShares US Property Yield, da BlackRock, linha que distribui. Compra REITs, os fundos imobiliários dos EUA. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("JREG.L", "2026-11-13", "JPMorgan Global Research Enhanced Index Equity. Fundo ativo que segue de perto o MSCI World (países desenvolvidos), com pequenos desvios escolhidos pelos analistas da gestora. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("JEGA.L", "2026-11-13", "JPMorgan Global Equity Premium Income, linha que acumula. Fundo ativo de ações de países desenvolvidos que vende opções para gerar renda; em troca, ganha menos quando a bolsa sobe muito. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("JPGL.L", "2026-11-13", "JPMorgan Global Equity Multi-Factor. Compra ações de países desenvolvidos escolhidas por valor, qualidade e tendência de alta. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("HEAL.L", "2026-11-13", "iShares Healthcare Innovation, da BlackRock. Compra empresas de inovação em saúde do mundo todo (biotecnologia, genética, equipamentos médicos). É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("WCLD.L", "2026-11-13", "WisdomTree Cloud Computing. Compra empresas de software e serviços em nuvem, a maioria dos EUA. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("RBOT.L", "2026-11-13", "iShares Automation & Robotics, da BlackRock. Compra empresas de automação, robótica e inteligência artificial do mundo todo. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("SMH.L", "2026-11-13", "VanEck Semiconductor. Compra as maiores empresas de semicondutores (chips) do mundo, com peso grande em poucas empresas. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("DGRA.L", "2026-11-13", "WisdomTree US Quality Dividend Growth. Compra empresas dos EUA que pagam dividendos e têm lucro de qualidade e chance de aumentar os dividendos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("GGRA.L", "2026-11-13", "WisdomTree Global Quality Dividend Growth. Compra empresas do mundo todo que pagam dividendos e têm lucro de qualidade e chance de aumentar os dividendos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar."),
    ("VHYA.L", "2026-11-13", "Vanguard FTSE All-World High Dividend Yield, linha que acumula. Compra empresas do mundo todo que pagam dividendos acima da média. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Não paga proventos: reinveste tudo no próprio fundo."),
    ("VHYD.L", "2026-11-13", "Vanguard FTSE All-World High Dividend Yield, linha que distribui. Compra empresas do mundo todo que pagam dividendos acima da média. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("TDIV.L", "2026-11-13", "VanEck Morningstar Developed Markets Dividend Leaders. Compra as 100 empresas de países desenvolvidos com os maiores dividendos que parecem sustentáveis. É um fundo dos Países Baixos (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("UDVD.L", "2026-11-13", "SPDR S&P US Dividend Aristocrats, da State Street. Compra empresas dos EUA que aumentam os dividendos há pelo menos 20 anos seguidos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("GLDV.L", "2026-11-13", "SPDR S&P Global Dividend Aristocrats, da State Street. Compra empresas do mundo todo com dividendos altos e estáveis há pelo menos 10 anos. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("WQDV.L", "2026-11-13", "iShares MSCI World Quality Dividend, da BlackRock, linha que distribui. Compra empresas de países desenvolvidos com dividendos acima da média e lucro de qualidade. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("FUSD.L", "2026-11-13", "Fidelity US Quality Income, linha que distribui. Compra empresas dos EUA com lucro de qualidade e dividendos acima da média. É um fundo da Irlanda (UCITS), negociado na Bolsa de Londres em dólar. Paga os proventos em dinheiro, sem o imposto de 30% retido nos EUA."),
    ("FBTC.L", "2026-11-13", "Fidelity Physical Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa da Alemanha, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("BTIC.L", "2026-11-13", "Invesco Physical Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa de Jersey, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("BTCW.L", "2026-11-13", "WisdomTree Physical Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa de Jersey, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("BITC.L", "2026-11-13", "CoinShares Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa de Jersey, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("CBTU.L", "2026-11-13", "21Shares Bitcoin Core. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa da Suíça, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("BTCU.L", "2026-11-13", "21Shares Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa da Suíça, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("BTC1.L", "2026-11-13", "Bitwise Core Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa da Alemanha, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    ("BTCF.L", "2026-11-13", "Bitwise Physical Bitcoin. Acompanha o preço do bitcoin: cada cota é lastreada em bitcoins guardados num custodiante. Não é um fundo: é uma nota do emissor (ETN), de uma empresa da Alemanha, negociada na Bolsa de Londres em dólar. Se o emissor quebrar, a garantia são os bitcoins guardados. Não paga proventos."),
    // EUA.
    ("VOO", "2026-11-12", texto!("Vanguard S&P 500. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("IVV", "2026-11-12", texto!("iShares Core S&P 500, da BlackRock. Segue o S&P 500, as cerca de 500 maiores empresas dos EUA.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("SPY", "2026-11-12", texto!("SPDR S&P 500, da State Street, o ETF mais antigo do S&P 500. Segue as cerca de 500 maiores empresas dos EUA.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VTI", "2026-11-12", texto!("Vanguard Total Stock Market. Segue o mercado de ações americano inteiro (índice CRSP US Total Market): empresas grandes, médias e pequenas.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VT", "2026-11-12", texto!("Vanguard Total World Stock. Segue o FTSE Global All Cap: ações do mundo todo, de países desenvolvidos e emergentes.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("QQQ", "2026-11-12", texto!("Invesco QQQ. Segue o Nasdaq-100, as 100 maiores empresas não financeiras da bolsa Nasdaq, com peso grande de tecnologia.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VNQ", "2026-11-12", texto!("Vanguard Real Estate. Investe em REITs, os fundos imobiliários dos EUA (escritórios, galpões, torres, residências e outros).", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VIG", "2026-11-12", texto!("Vanguard Dividend Appreciation. Segue o S&P U.S. Dividend Growers: empresas americanas que aumentam os dividendos há pelo menos 10 anos seguidos.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("VYM", "2026-11-12", texto!("Vanguard High Dividend Yield. Segue o FTSE High Dividend Yield: empresas americanas que pagam dividendos acima da média.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("SCHD", "2026-11-12", texto!("Schwab U.S. Dividend Equity. Segue o Dow Jones U.S. Dividend 100: 100 empresas americanas com dividendos altos e constantes.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("GLD", "2026-11-12", "SPDR Gold Shares, da State Street. Acompanha o preço do ouro: as cotas são lastreadas em ouro físico guardado em cofre. É um fundo dos EUA, negociado em dólar, e não paga proventos."),
    ("IAU", "2026-11-12", "iShares Gold Trust, da BlackRock. Acompanha o preço do ouro: as cotas são lastreadas em ouro físico guardado em cofre. É um fundo dos EUA, negociado em dólar, e não paga proventos."),
    ("IBIT", "2026-11-12", "iShares Bitcoin Trust, da BlackRock. Acompanha o preço do bitcoin: o fundo guarda bitcoins com um custodiante. É um fundo dos EUA, negociado em dólar, e não paga proventos."),
    ("EWZ", "2026-11-12", texto!("iShares MSCI Brazil, da BlackRock. Segue o MSCI Brazil 25/50: grandes e médias empresas brasileiras, cotadas em dólar.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("BND", "2026-11-12", texto!("Vanguard Total Bond Market. Compra títulos de renda fixa dos EUA com grau de investimento: Tesouro americano, empresas e hipotecas.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    ("TLT", "2026-11-12", texto!("iShares 20+ Year Treasury Bond, da BlackRock. Compra títulos do Tesouro dos EUA que vencem em mais de 20 anos: oscila bastante com os juros americanos.", "É um fundo dos EUA, negociado em dólar. Os proventos chegam com 30% de imposto retido lá.")),
    // B3.
    ("IVVB11", "2026-11-12", "iShares S&P 500 Fundo de Índice, da BlackRock. Investe no ETF americano IVV, que segue o S&P 500, e é negociado em reais na B3: sobe e desce com as empresas americanas e com o dólar. Não paga proventos: reinveste tudo."),
    ("BOVA11", "2026-11-12", "iShares Ibovespa Fundo de Índice, da BlackRock. Segue o Ibovespa, o principal índice de ações da B3."),
    ("SMAL11", "2026-11-12", "iShares BM&FBOVESPA Small Cap, da BlackRock. Segue o índice SMLL, das empresas menores da B3."),
    ("DIVO11", "2026-11-12", "It Now IDIV, do Itaú. Segue o IDIV, índice das empresas da B3 que mais pagam dividendos."),
    ("HASH11", "2026-11-12", "Hashdex Nasdaq Crypto Index. Segue o Nasdaq Crypto Index, uma cesta das maiores criptomoedas (bitcoin, ethereum e outras), em reais na B3."),
    ("GOLD11", "2026-11-12", "Trend ETF LBMA Ouro. Acompanha o preço do ouro em reais, negociado na B3."),
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
        assert!(sobre("cspx.l").unwrap().texto.contains("Irlanda"));
        assert!(sobre("VOO").unwrap().texto.contains("30%"));
        assert_eq!(sobre("XXXX"), None);
        assert_eq!(setor("financial_services"), Some("Financeiro"));
        assert_eq!(setor("Consumer Cyclical"), Some("Consumo não essencial"));
        assert_eq!(setor("outro"), None);
        // Cada texto da lista está completo e cita o que o fundo faz.
        for (k, d, v) in TEXTOS {
            assert!(
                chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").is_ok(),
                "{k}: data {d}"
            );
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
