//! E-mail com a chave de recuperação, aberto no programa de e-mail do próprio
//! usuário (link `mailto:`). O aplicativo não envia nada pela internet.

const SUBJECT: &str = "4Nier Patrimônio Auditável: chave de recuperação";

pub fn body(key: &str, generated_at: &str) -> String {
    format!(
        "Esta é a chave de recuperação do seu 4Nier Patrimônio Auditável:\n\
         \n\
         {key}\n\
         \n\
         Guarde este e-mail. Se esquecer a senha, abra o aplicativo, clique em \
         \"Esqueci minha senha\" e digite esta chave para criar uma senha nova.\n\
         \n\
         A chave vale também para os backups feitos a partir de agora. Se você \
         gerar uma chave nova no aplicativo, esta deixa de valer.\n\
         \n\
         Cuidado: quem tiver esta chave e uma cópia dos seus dados consegue abrir \
         sua carteira. Não encaminhe este e-mail.\n\
         \n\
         Gerada em {generated_at}."
    )
}

/// Codifica tudo que não é letra, número ou `-._~`, como pede o `mailto:`.
fn encode(text: &str) -> String {
    let mut out = String::new();
    for b in text.replace("\r\n", "\n").replace('\n', "\r\n").bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

pub fn mailto(email: &str, key: &str, generated_at: &str) -> String {
    format!(
        "mailto:{}?subject={}&body={}",
        encode(email.trim()).replace("%40", "@"),
        encode(SUBJECT),
        encode(&body(key, generated_at))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_mailto_codificado() {
        let url = mailto("eu+teste@exemplo.com", "ABCD-EFGH", "15/10/2026 às 10:00");
        assert!(url.starts_with("mailto:eu%2Bteste@exemplo.com?subject=4Nier%20Patrim%C3%B4nio%20Audit%C3%A1vel"));
        assert!(url.contains("ABCD-EFGH"));
        assert!(url.contains("%0D%0A"));
        assert!(!url.contains(' '));
        assert!(!url.contains('\n'));
        // Cabe no limite de 2 mil caracteres do Windows para links.
        assert!(url.len() < 2000, "{}", url.len());
    }
}
