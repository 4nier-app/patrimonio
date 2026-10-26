//! Fundos negociados na B3 que o catálogo diário (COTAHIST) não separa dos
//! ETFs de ações, mas têm outra regra de imposto. A lista vem das gestoras e
//! dos sites de listagem e é revista a cada versão.

/// FI-Infra (Lei 12.431, art. 3º): para pessoa física, rendimentos e ganho na
/// venda são isentos.
pub const FI_INFRA_KIND: &str = "FI_INFRA";
/// ETF de renda fixa (Lei 13.043, art. 2º): o imposto é retido na fonte pelo
/// administrador na venda; não há DARF.
pub const ETF_RF_KIND: &str = "ETF_RF";

const FI_INFRA: &[&str] = &[
    "BCDI11", "BDIF11", "BIDB11", "BINC11", "BISE11", "BODB11", "BODI11", "CDII11", "CPDI11",
    "CPTI11", "DBIN11", "DIVS11", "EXIF11", "IFRA11", "IFRI11", "INFB11", "IRIF11", "ISEN11",
    "ISET11", "ISNN11", "ISNT11", "ISTT11", "JMBI11", "JURO11", "KDIF11", "NUIF11", "OGIN11",
    "PREE11", "PRIF11", "RBIF11", "RIFF11", "SNID11", "SUIN11", "VALO11", "VANG11", "VINF11",
    "XPID11",
];

const ETF_RF: &[&str] = &[
    "B5MB11", "B5P211", "BDAP11", "DEBB11", "FIXA11", "IB5M11", "IMAB11", "IMBB11", "IRFM11",
    "LFTB11", "LFTS11", "NTNS11", "PACB11", "PACC11", "PACG11", "POSB11",
];

/// Classe própria do ativo, quando ele está numa das listas.
pub fn special_kind(ticker: &str) -> Option<&'static str> {
    let t = ticker.trim().to_uppercase();
    if FI_INFRA.contains(&t.as_str()) {
        Some(FI_INFRA_KIND)
    } else if ETF_RF.contains(&t.as_str()) {
        Some(ETF_RF_KIND)
    } else {
        None
    }
}

/// Todos os ativos das listas, com a classe de cada um.
pub fn all() -> impl Iterator<Item = (&'static str, &'static str)> {
    FI_INFRA
        .iter()
        .map(|t| (*t, FI_INFRA_KIND))
        .chain(ETF_RF.iter().map(|t| (*t, ETF_RF_KIND)))
}

/// Venda que não entra no DARF mensal: isenta (FI-Infra) ou com o imposto
/// retido pelo próprio fundo (ETF de renda fixa).
pub fn outside_darf(kind: &str) -> bool {
    kind == FI_INFRA_KIND || kind == ETF_RF_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_as_listas() {
        assert_eq!(special_kind("KDIF11"), Some(FI_INFRA_KIND));
        assert_eq!(special_kind("imab11"), Some(ETF_RF_KIND));
        assert_eq!(special_kind("BOVA11"), None);
        assert!(outside_darf(FI_INFRA_KIND) && outside_darf(ETF_RF_KIND));
        assert!(!outside_darf("ETF"));
        let mut seen = std::collections::BTreeSet::new();
        assert!(all().all(|(t, _)| seen.insert(t)), "ticker repetido");
    }
}
