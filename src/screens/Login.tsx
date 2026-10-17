import { useState, type FormEvent } from "react";
import { api, errorMessage, type VaultStatus } from "../api";
import { passwordStrength } from "../format";
import PasswordInput from "../components/PasswordInput";
import Logo from "../components/Logo";
import RecoveryKeyCard from "../components/RecoveryKeyCard";
import RestoreBackup from "../components/RestoreBackup";

const STRENGTH = ["muito fraca", "fraca", "razoável", "boa", "forte"];

type Mode = "entrar" | "esqueci" | "restaurar";

export default function Login({
  status,
  onUnlocked,
  onProfile,
}: {
  status: VaultStatus;
  onUnlocked: () => void;
  /** O perfil mudou (trocado, criado, renomeado): recarregar o status. */
  onProfile: () => void;
}) {
  const creating = !status.exists;
  const profile = status.profiles.find((p) => p.id === status.profile);
  const [mode, setMode] = useState<Mode>("entrar");
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [email, setEmail] = useState("");
  const [recoveryKey, setRecoveryKey] = useState("");
  const [newKey, setNewKey] = useState<string | null>(null);
  const [restored, setRestored] = useState<string | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Edição de perfil: criar um novo ou renomear o atual.
  const [editing, setEditing] = useState<"novo" | "renomear" | null>(null);
  const [profileName, setProfileName] = useState("");

  async function profileAction(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try {
      await action();
      setEditing(null);
      setPassword("");
      setConfirmation("");
      onProfile();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function saveProfile() {
    const name = profileName;
    void profileAction(() =>
      editing === "novo" ? api.createProfile(name) : api.renameProfile(status.profile, name),
    );
  }

  function go(m: Mode) {
    setMode(m);
    setError(null);
    setPassword("");
    setConfirmation("");
  }

  async function submit(e: FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      if (mode === "esqueci") {
        await api.resetPassword(recoveryKey, password, confirmation);
        onUnlocked();
      } else if (creating) {
        setNewKey(await api.createVault(password, confirmation, email));
      } else {
        await api.unlock(password);
        onUnlocked();
      }
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  const strength = passwordStrength(password);
  const choosing = creating || mode === "esqueci";
  const mismatch = choosing && confirmation.length > 0 && confirmation !== password;

  // Depois de criar o cofre: mostrar a chave de recuperação antes de entrar.
  if (newKey) {
    return (
      <main className="center">
        <div className="card login">
          <RecoveryKeyCard recoveryKey={newKey} email={email} onDone={onUnlocked} doneLabel="Abrir minha carteira" />
        </div>
      </main>
    );
  }

  if (restored !== undefined) {
    return (
      <main className="center">
        <div className="card login">
          <h3>Backup restaurado</h3>
          <p className="ok">Os dados do backup foram conferidos e já estão em uso.</p>
          {restored && (
            <p className="muted">
              Os dados que estavam neste computador foram guardados em <span className="mono">{restored}</span>.
            </p>
          )}
          <button onClick={onUnlocked}>Abrir minha carteira</button>
        </div>
      </main>
    );
  }

  if (mode === "restaurar") {
    return (
      <main className="center">
        <div className="card login">
          <div className="login-brand">
            <Logo size={48} />
          </div>
          <h3>Restaurar um backup</h3>
          <RestoreBackup replacing={status.exists} onRestored={(p) => setRestored(p)} onCancel={() => go("entrar")} />
        </div>
      </main>
    );
  }

  return (
    <main className="center">
      <form className="card login" onSubmit={submit}>
        <div className="login-brand">
          <Logo size={48} />
        </div>
        {mode === "entrar" && (
          <div className="login-profile">
            {editing ? (
              <>
                <label>
                  {editing === "novo" ? "Nome do novo perfil" : "Nome deste perfil"}
                  <input
                    autoFocus
                    value={profileName}
                    maxLength={40}
                    placeholder="Ex.: Marcelo, Esposa"
                    onChange={(e) => setProfileName(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        saveProfile();
                      }
                    }}
                  />
                </label>
                <div className="login-links">
                  <button type="button" className="link" disabled={busy || !profileName.trim()} onClick={saveProfile}>
                    {editing === "novo" ? "Criar perfil" : "Salvar nome"}
                  </button>
                  <button type="button" className="link" onClick={() => setEditing(null)}>
                    Cancelar
                  </button>
                </div>
              </>
            ) : (
              <>
                {status.profiles.length > 1 ? (
                  <label>
                    Perfil
                    <select
                      value={status.profile}
                      disabled={busy}
                      onChange={(e) => {
                        const id = e.target.value;
                        void profileAction(() => api.selectProfile(id));
                      }}
                    >
                      {status.profiles.map((p) => (
                        <option key={p.id} value={p.id}>
                          {p.name}
                        </option>
                      ))}
                    </select>
                  </label>
                ) : (
                  <p className="muted small">Perfil: {profile?.name}</p>
                )}
                <div className="login-links">
                  <button
                    type="button"
                    className="link"
                    onClick={() => {
                      setProfileName("");
                      setEditing("novo");
                    }}
                  >
                    Novo perfil
                  </button>
                  <button
                    type="button"
                    className="link"
                    onClick={() => {
                      setProfileName(profile?.name ?? "");
                      setEditing("renomear");
                    }}
                  >
                    Renomear
                  </button>
                  {creating && status.profile !== "principal" && (
                    <button
                      type="button"
                      className="link"
                      onClick={() => void profileAction(() => api.removeProfile(status.profile))}
                    >
                      Remover este perfil
                    </button>
                  )}
                </div>
              </>
            )}
          </div>
        )}
        {mode === "esqueci" ? (
          <>
            <h3>Esqueci minha senha</h3>
            {status.has_recovery_key ? (
              <p className="muted">
                Digite a chave de recuperação (procure no seu e-mail por "chave de recuperação") e
                escolha uma senha nova.
              </p>
            ) : (
              <p className="warning">
                Esta carteira ainda não tem chave de recuperação, então não há como criar uma senha nova. Se você tiver um
                backup e lembrar a senha da época dele, use "Restaurar um backup".
              </p>
            )}
            <label>
              Chave de recuperação
              <input
                className="mono"
                autoFocus
                autoComplete="off"
                spellCheck={false}
                value={recoveryKey}
                onChange={(e) => setRecoveryKey(e.target.value.toUpperCase())}
                placeholder="XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
              />
            </label>
          </>
        ) : creating ? (
          <>
            <p className="muted">
              Crie a senha que protege os dados {status.profiles.length > 1 ? `do perfil ${profile?.name}` : "deste perfil"}.
              Eles ficam criptografados neste computador e não são enviados para lugar nenhum. Cada perfil tem a própria
              senha e não vê os dados dos outros.
            </p>
            <p className="muted">
              Em seguida o aplicativo gera uma <strong>chave de recuperação</strong>, para o caso de você esquecer a senha,
              e ajuda você a mandá-la para o seu e-mail.
            </p>
          </>
        ) : (
          <p className="muted">Digite sua senha para abrir a carteira.</p>
        )}

        <PasswordInput
          label={mode === "esqueci" ? "Nova senha" : "Senha"}
          autoFocus={mode !== "esqueci"}
          value={password}
          onChange={setPassword}
        />
        {choosing && (
          <>
            <div className="strength" aria-live="polite">
              <div className={`bar s${strength}`} />
              <span>Força: {STRENGTH[strength]}</span>
            </div>
            <PasswordInput label="Confirme a senha" value={confirmation} onChange={setConfirmation} />
            {mismatch && <p className="error">As duas senhas estão diferentes. Use "Mostrar" para conferir.</p>}
          </>
        )}
        {creating && mode === "entrar" && (
          <label>
            Seu e-mail (para receber a chave de recuperação)
            <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="voce@exemplo.com" />
          </label>
        )}

        {error && <p className="error">{error}</p>}
        <button
          type="submit"
          disabled={
            busy ||
            password.length === 0 ||
            mismatch ||
            (mode === "esqueci" && (!status.has_recovery_key || !recoveryKey.trim() || !confirmation))
          }
        >
          {busy ? "Aguarde…" : mode === "esqueci" ? "Criar senha nova e entrar" : creating ? "Criar cofre" : "Entrar"}
        </button>

        <div className="login-links">
          {mode === "esqueci" ? (
            <button type="button" className="link" onClick={() => go("entrar")}>
              Voltar
            </button>
          ) : (
            !creating && (
              <button type="button" className="link" onClick={() => go("esqueci")}>
                Esqueci minha senha
              </button>
            )
          )}
          <button type="button" className="link" onClick={() => go("restaurar")}>
            Restaurar um backup
          </button>
        </div>
        <p className="footnote">Versão {status.app_version}</p>
      </form>
    </main>
  );
}
