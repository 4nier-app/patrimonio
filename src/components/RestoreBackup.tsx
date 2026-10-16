import { useState, type FormEvent } from "react";
import { api, errorMessage, type BackupSummary } from "../api";
import { formatDateTime } from "../format";
import PasswordInput from "./PasswordInput";

type Props = {
  /** Há um cofre nesta pasta que será substituído (e guardado). */
  replacing: boolean;
  onRestored: (previous: string | null) => void;
  onCancel?: () => void;
};

/** Escolher um arquivo de backup, conferir o que tem nele e restaurar com a senha. */
export default function RestoreBackup({ replacing, onRestored, onCancel }: Props) {
  const [summary, setSummary] = useState<BackupSummary | null>(null);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pick() {
    setError(null);
    try {
      const s = await api.pickBackup();
      if (s) setSummary(s);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  async function restore(e: FormEvent) {
    e.preventDefault();
    if (!summary) return;
    setBusy(true);
    setError(null);
    try {
      const r = await api.restoreBackup(summary.path, password);
      onRestored(r.previous);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form className="restore" onSubmit={restore}>
      {!summary ? (
        <>
          <p className="muted">Escolha o arquivo de backup (termina em .cabackup).</p>
          <button type="button" onClick={pick}>
            Escolher arquivo…
          </button>
        </>
      ) : (
        <>
          <dl className="summary">
            <dt>Arquivo</dt>
            <dd className="mono">{summary.path}</dd>
            <dt>Feito em</dt>
            <dd>{formatDateTime(summary.created_at)}</dd>
            <dt>Versão do aplicativo</dt>
            <dd>{summary.app_version}</dd>
            <dt>Registros no histórico</dt>
            <dd>{summary.events}</dd>
          </dl>
          <PasswordInput label="Senha da carteira na data do backup" autoFocus value={password} onChange={setPassword} />
          {replacing && (
            <p className="warning">
              O backup vai substituir os dados atuais. Os dados atuais não são apagados: ficam guardados na pasta
              "anteriores", dentro da pasta de dados.
            </p>
          )}
          <div className="actions">
            <button type="submit" disabled={busy || !password}>
              {busy ? "Conferindo e restaurando…" : "Restaurar"}
            </button>
            <button type="button" className="secondary" onClick={pick} disabled={busy}>
              Outro arquivo
            </button>
          </div>
        </>
      )}
      {error && <p className="error">{error}</p>}
      {onCancel && (
        <button type="button" className="link" onClick={onCancel}>
          Voltar
        </button>
      )}
    </form>
  );
}
