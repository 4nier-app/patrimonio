import { useState, type FormEvent } from "react";
import { api, errorMessage, type VaultStatus } from "../api";
import { passwordStrength } from "../format";
import PasswordInput from "../components/PasswordInput";

const STRENGTH = ["muito fraca", "fraca", "razoável", "boa", "forte"];

export default function Login({ status, onUnlocked }: { status: VaultStatus; onUnlocked: () => void }) {
  const creating = !status.exists;
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      if (creating) await api.createVault(password, confirmation);
      else await api.unlock(password);
      onUnlocked();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const strength = passwordStrength(password);
  const mismatch = creating && confirmation.length > 0 && confirmation !== password;

  return (
    <main className="center">
      <form className="card login" onSubmit={submit}>
        <h1>Carteira Auditável</h1>
        {creating ? (
          <>
            <p className="muted">
              Crie a senha que protege seus dados. Eles ficam criptografados neste computador e não são enviados
              para lugar nenhum.
            </p>
            <p className="warning">
              Não existe recuperação de senha. Se você esquecer a senha, os dados não podem ser abertos. Guarde-a em
              local seguro.
            </p>
          </>
        ) : (
          <p className="muted">Digite sua senha para abrir a carteira.</p>
        )}

        <PasswordInput label="Senha" autoFocus value={password} onChange={setPassword} />
        {creating && (
          <>
            <div className="strength" aria-live="polite">
              <div className={`bar s${strength}`} />
              <span>Força: {STRENGTH[strength]}</span>
            </div>
            <PasswordInput label="Confirme a senha" value={confirmation} onChange={setConfirmation} />
            {mismatch && <p className="error">As duas senhas estão diferentes. Use "Mostrar" para conferir.</p>}
          </>
        )}

        {error && <p className="error">{error}</p>}
        <button type="submit" disabled={busy || password.length === 0 || mismatch}>
          {busy ? "Aguarde…" : creating ? "Criar cofre" : "Entrar"}
        </button>
        <p className="footnote">Versão {status.app_version}</p>
      </form>
    </main>
  );
}
