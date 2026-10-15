import { useEffect, useState, type FormEvent } from "react";
import { api, errorMessage, type BackupInfo, type BackupStatus, type RecoveryStatus, type VaultStatus } from "../api";
import { date, formatDateTime } from "../format";
import PasswordInput from "../components/PasswordInput";
import RecoveryKeyCard from "../components/RecoveryKeyCard";
import RestoreBackup from "../components/RestoreBackup";

type Msg = { ok: boolean; text: string } | null;

type Props = { status: VaultStatus; onRestored: () => void; onChanged: () => void };

export default function Security({ status, onRestored, onChanged }: Props) {
  return (
    <section>
      <h2>Segurança</h2>
      <p className="muted">
        Seus dados ficam criptografados (SQLCipher, AES-256) na pasta abaixo. A chave do banco é protegida pela sua senha
        (Argon2id) e, se você quiser, também por uma chave de recuperação. Nenhuma das duas é gravada.
      </p>
      <p className="mono path">{status.data_dir}</p>

      <div className="security-grid">
        <Recovery onChanged={onChanged} />
        <Backup onRestored={onRestored} />
        <ChangePassword />
      </div>
    </section>
  );
}

function Recovery({ onChanged }: { onChanged: () => void }) {
  const [st, setSt] = useState<RecoveryStatus | null>(null);
  const [email, setEmail] = useState("");
  const [key, setKey] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [message, setMessage] = useState<Msg>(null);

  function load() {
    api
      .recoveryStatus()
      .then((s) => {
        setSt(s);
        setEmail(s.email ?? "");
      })
      .catch((e) => setMessage({ ok: false, text: errorMessage(e) }));
  }
  useEffect(load, []);

  async function generate() {
    setMessage(null);
    try {
      setKey(await api.generateRecoveryKey(email));
      setConfirming(false);
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    }
  }

  if (key) {
    return (
      <div className="card">
        <RecoveryKeyCard
          recoveryKey={key}
          email={email}
          doneLabel="Concluir"
          onDone={() => {
            setKey(null);
            load();
            onChanged();
            setMessage({ ok: true, text: "Chave de recuperação ativa." });
          }}
        />
      </div>
    );
  }

  return (
    <div className="card stack">
      <h3>Chave de recuperação</h3>
      {st?.configured ? (
        <p className="ok">
          Ativa{st.created_at ? ` desde ${formatDateTime(st.created_at)}` : ""}
          {st.email ? `, enviada para ${st.email}` : ""}.
        </p>
      ) : (
        <p className="warning">
          Você ainda não tem chave de recuperação. Sem ela, se esquecer a senha, não há como abrir a carteira.
        </p>
      )}
      <p className="muted">
        Com a chave, a opção "Esqueci minha senha" da tela de entrada cria uma senha nova sem perder nada.
      </p>
      <label>
        Seu e-mail
        <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="voce@exemplo.com" />
      </label>
      {st?.configured && !confirming ? (
        <button className="secondary" onClick={() => setConfirming(true)}>
          Gerar nova chave
        </button>
      ) : (
        <>
          {st?.configured && (
            <p className="warning">A chave atual deixa de valer assim que a nova for gerada. Continuar?</p>
          )}
          <div className="actions">
            <button onClick={generate}>{st?.configured ? "Sim, gerar nova chave" : "Gerar chave de recuperação"}</button>
            {confirming && (
              <button className="secondary" onClick={() => setConfirming(false)}>
                Cancelar
              </button>
            )}
          </div>
        </>
      )}
      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}
    </div>
  );
}

function Backup({ onRestored }: { onRestored: () => void }) {
  const [st, setSt] = useState<BackupStatus | null>(null);
  const [done, setDone] = useState<BackupInfo | null>(null);
  const [restoring, setRestoring] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<Msg>(null);

  function load() {
    api.backupStatus().then(setSt).catch((e) => setMessage({ ok: false, text: errorMessage(e) }));
  }
  useEffect(load, []);

  async function exportNow() {
    setMessage(null);
    setDone(null);
    setBusy(true);
    try {
      const info = await api.exportBackup();
      if (info) {
        setDone(info);
        load();
      }
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="card stack">
      <h3>Backup</h3>
      <p className="muted">
        O backup é um arquivo único com todos os seus dados, ainda criptografado. Guarde uma cópia fora deste computador
        (pendrive, Google Drive, OneDrive). Para restaurá-lo, use a senha que valia no dia do backup.
      </p>
      <button onClick={exportNow} disabled={busy}>
        {busy ? "Gravando…" : "Fazer backup agora…"}
      </button>
      {done && (
        <p className="ok">
          Backup gravado em <span className="mono">{done.path}</span> ({(done.bytes / 1024 / 1024).toFixed(1)} MB).
        </p>
      )}
      {st && (
        <p className="muted">
          Último backup: {st.last_backup ? formatDateTime(st.last_backup) : "nenhum"}. O aplicativo também faz um backup
          automático por semana na pasta de dados (guarda os 5 mais recentes)
          {st.last_auto ? `; o último foi em ${date(st.last_auto)}` : ""}.{" "}
          <button className="link" onClick={() => api.openDataFolder().catch(() => undefined)}>
            Abrir pasta
          </button>
        </p>
      )}
      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}

      <h4>Restaurar</h4>
      {restoring ? (
        <RestoreBackup replacing onRestored={() => onRestored()} onCancel={() => setRestoring(false)} />
      ) : (
        <button className="secondary" onClick={() => setRestoring(true)}>
          Restaurar um backup…
        </button>
      )}
    </div>
  );
}

function ChangePassword() {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [message, setMessage] = useState<Msg>(null);

  async function submit(e: FormEvent) {
    e.preventDefault();
    setMessage(null);
    try {
      await api.changePassword(current, next, confirmation);
      setCurrent("");
      setNext("");
      setConfirmation("");
      setMessage({ ok: true, text: "Senha alterada. Use a nova senha a partir de agora." });
    } catch (err) {
      setMessage({ ok: false, text: errorMessage(err) });
    }
  }

  return (
    <form className="card stack" onSubmit={submit}>
      <h3>Trocar senha</h3>
      <PasswordInput label="Senha atual" value={current} onChange={setCurrent} />
      <PasswordInput label="Nova senha" value={next} onChange={setNext} />
      <PasswordInput label="Confirme a nova senha" value={confirmation} onChange={setConfirmation} />
      <p className="muted">A chave de recuperação continua valendo depois da troca.</p>
      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}
      <button type="submit" disabled={!current || !next}>
        Trocar senha
      </button>
    </form>
  );
}
