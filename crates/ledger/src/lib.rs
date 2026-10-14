//! Log de eventos imutável (append-only) com cadeia de hash.
//!
//! Cada evento guarda o hash do evento anterior. Qualquer alteração em um
//! evento já gravado muda o seu hash e quebra a cadeia a partir dele, o que
//! [`verify_chain`] detecta e informa.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Hash usado como "anterior" do primeiro evento da cadeia.
pub const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

/// Ação registrada sobre uma entidade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Action {
    Criada,
    Alterada,
    Removida,
}

impl Action {
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Criada => "CRIADA",
            Action::Alterada => "ALTERADA",
            Action::Removida => "REMOVIDA",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "CRIADA" => Some(Action::Criada),
            "ALTERADA" => Some(Action::Alterada),
            "REMOVIDA" => Some(Action::Removida),
            _ => None,
        }
    }
}

/// Dados de um evento antes de entrar na cadeia.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NewEvent {
    /// Data/hora em ISO 8601 (UTC).
    pub occurred_at: String,
    /// Tipo de entidade, ex.: "OPERACAO", "SISTEMA".
    pub entity: String,
    pub entity_id: String,
    pub action: Action,
    /// Estado anterior (JSON) ou `None` em criações.
    pub before: Option<String>,
    /// Estado posterior (JSON) ou `None` em remoções.
    pub after: Option<String>,
    pub reason: Option<String>,
    pub app_version: String,
}

/// Evento já encadeado.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub seq: i64,
    #[serde(flatten)]
    pub data: NewEvent,
    pub prev_hash: String,
    pub hash: String,
}

/// Calcula o hash de um evento.
///
/// Cada campo entra com o seu tamanho em bytes na frente, para que nenhuma
/// combinação de campos diferentes produza a mesma sequência de bytes.
/// Campos ausentes (`None`) entram como tamanho `-1`, diferente de texto vazio.
pub fn compute_hash(seq: i64, data: &NewEvent, prev_hash: &str) -> String {
    let mut h = Sha256::new();
    let mut field = |value: Option<&str>| match value {
        Some(v) => {
            h.update((v.len() as i64).to_be_bytes());
            h.update(v.as_bytes());
        }
        None => h.update((-1i64).to_be_bytes()),
    };
    let seq_str = seq.to_string();
    field(Some(&seq_str));
    field(Some(&data.occurred_at));
    field(Some(&data.entity));
    field(Some(&data.entity_id));
    field(Some(data.action.as_str()));
    field(data.before.as_deref());
    field(data.after.as_deref());
    field(data.reason.as_deref());
    field(Some(&data.app_version));
    field(Some(prev_hash));
    hex::encode(h.finalize())
}

/// Encadeia um novo evento depois de `previous` (ou no início da cadeia).
pub fn chain(previous: Option<&Event>, data: NewEvent) -> Event {
    let (seq, prev_hash) = match previous {
        Some(p) => (p.seq + 1, p.hash.clone()),
        None => (1, GENESIS_HASH.to_string()),
    };
    let hash = compute_hash(seq, &data, &prev_hash);
    Event { seq, data, prev_hash, hash }
}

/// Problema encontrado na verificação da cadeia.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
pub enum ChainError {
    #[error("evento {seq}: sequência esperada {expected}")]
    SequenceGap { seq: i64, expected: i64 },
    #[error("evento {seq}: hash anterior não confere com o evento {}", seq - 1)]
    BrokenLink { seq: i64 },
    #[error("evento {seq}: conteúdo foi alterado depois de gravado")]
    Tampered { seq: i64 },
}

/// Verifica a cadeia inteira, em ordem. Retorna o primeiro problema encontrado.
pub fn verify_chain<'a>(events: impl IntoIterator<Item = &'a Event>) -> Result<usize, ChainError> {
    let mut expected_seq = 1;
    let mut prev_hash = GENESIS_HASH.to_string();
    let mut count = 0;
    for e in events {
        if e.seq != expected_seq {
            return Err(ChainError::SequenceGap { seq: e.seq, expected: expected_seq });
        }
        if e.prev_hash != prev_hash {
            return Err(ChainError::BrokenLink { seq: e.seq });
        }
        if compute_hash(e.seq, &e.data, &e.prev_hash) != e.hash {
            return Err(ChainError::Tampered { seq: e.seq });
        }
        prev_hash = e.hash.clone();
        expected_seq += 1;
        count += 1;
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(n: u32) -> NewEvent {
        NewEvent {
            occurred_at: format!("2026-01-{:02}T10:00:00Z", n),
            entity: "OPERACAO".into(),
            entity_id: format!("op-{n}"),
            action: Action::Criada,
            before: None,
            after: Some(format!(r#"{{"quantidade":"{n}"}}"#)),
            reason: None,
            app_version: "0.1.0".into(),
        }
    }

    fn build(n: u32) -> Vec<Event> {
        let mut out: Vec<Event> = Vec::new();
        for i in 1..=n {
            let e = chain(out.last(), sample(i));
            out.push(e);
        }
        out
    }

    #[test]
    fn cadeia_valida_e_verificada() {
        let events = build(5);
        assert_eq!(events[0].prev_hash, GENESIS_HASH);
        assert_eq!(events[4].seq, 5);
        assert_eq!(verify_chain(&events), Ok(5));
    }

    #[test]
    fn alteracao_de_conteudo_e_detectada() {
        let mut events = build(4);
        events[2].data.after = Some(r#"{"quantidade":"999"}"#.into());
        assert_eq!(verify_chain(&events), Err(ChainError::Tampered { seq: 3 }));
    }

    #[test]
    fn recalcular_hash_alterado_quebra_o_elo_seguinte() {
        let mut events = build(4);
        events[1].data.reason = Some("forjado".into());
        events[1].hash = compute_hash(2, &events[1].data, &events[1].prev_hash);
        assert_eq!(verify_chain(&events), Err(ChainError::BrokenLink { seq: 3 }));
    }

    #[test]
    fn remocao_de_evento_e_detectada() {
        let mut events = build(4);
        events.remove(1);
        assert_eq!(verify_chain(&events), Err(ChainError::SequenceGap { seq: 3, expected: 2 }));
    }

    #[test]
    fn ausente_difere_de_vazio() {
        let mut a = sample(1);
        let mut b = sample(1);
        a.reason = None;
        b.reason = Some(String::new());
        assert_ne!(compute_hash(1, &a, GENESIS_HASH), compute_hash(1, &b, GENESIS_HASH));
    }
}
