import { useState, type FormEvent } from "react";
import { api, errorMessage, type VaultStatus } from "../api";
import PasswordInput from "../components/PasswordInput";

export default function Security({ status }: { status: VaultStatus }) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

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
    <section>
      <h2>Segurança</h2>
      <p className="muted">
        Seus dados ficam criptografados (SQLCipher, AES-256) na pasta abaixo. A chave é derivada da sua senha com
        Argon2id e nunca é gravada.
      </p>
      <p className="mono path">{status.data_dir}</p>

      <form className="card narrow" onSubmit={submit}>
        <h3>Trocar senha</h3>
        <PasswordInput label="Senha atual" value={current} onChange={setCurrent} />
        <PasswordInput label="Nova senha" value={next} onChange={setNext} />
        <PasswordInput label="Confirme a nova senha" value={confirmation} onChange={setConfirmation} />
        {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}
        <button type="submit" disabled={!current || !next}>Trocar senha</button>
      </form>
    </section>
  );
}
