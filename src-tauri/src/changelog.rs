//! Changelog embutido no executável, mostrado no histórico do sistema.

const CHANGELOG: &str = include_str!("../../CHANGELOG.md");

/// Texto da seção `## [versão]` do CHANGELOG.md, sem o título.
pub fn section(version: &str) -> String {
    extract(CHANGELOG, version)
}

fn extract(text: &str, version: &str) -> String {
    let header = format!("## [{version}]");
    let mut lines = text.lines().skip_while(|l| !l.starts_with(&header));
    if lines.next().is_none() {
        return format!("Versão {version} (sem notas no changelog).");
    }
    lines
        .take_while(|l| !l.starts_with("## ["))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# Changelog\n\n## [0.2.0] - 2026-11-01\n### Adicionado\n- B\n\n## [0.1.0] - 2026-10-14\n- A\n";

    #[test]
    fn extrai_apenas_a_secao_da_versao() {
        assert_eq!(extract(SAMPLE, "0.2.0"), "### Adicionado\n- B");
        assert_eq!(extract(SAMPLE, "0.1.0"), "- A");
    }

    #[test]
    fn versao_ausente() {
        assert!(extract(SAMPLE, "9.9.9").contains("sem notas"));
    }

    #[test]
    fn changelog_real_tem_a_versao_atual() {
        assert!(!section(env!("CARGO_PKG_VERSION")).contains("sem notas"));
    }
}
