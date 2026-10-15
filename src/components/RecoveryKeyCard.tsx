import { useState } from "react";
import { api, errorMessage } from "../api";

type Props = {
  recoveryKey: string;
  email: string;
  onDone: () => void;
  doneLabel?: string;
};

/**
 * Mostra a chave de recuperação recém-gerada. Ela não fica gravada em lugar
 * nenhum: o usuário precisa mandá-la para o próprio e-mail ou anotá-la agora.
 */
export default function RecoveryKeyCard({ recoveryKey, email: initialEmail, onDone, doneLabel = "Continuar" }: Props) {
  const [email, setEmail] = useState(initialEmail);
  const [saved, setSaved] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  async function sendEmail() {
    setMessage(null);
    try {
      await api.sendRecoveryEmail(email, recoveryKey);
      setMessage({
        ok: true,
        text: "Seu programa de e-mail foi aberto com a mensagem pronta. Confira e clique em Enviar.",
      });
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    }
  }

  async function copy() {
    setMessage(null);
    try {
      await navigator.clipboard.writeText(recoveryKey);
      setMessage({ ok: true, text: "Chave copiada. Cole num lugar seguro." });
    } catch {
      setMessage({ ok: false, text: "Não foi possível copiar. Selecione a chave e copie com Ctrl+C." });
    }
  }

  return (
    <div className="recovery-card">
      <h3>Sua chave de recuperação</h3>
      <p className="muted">
        Se um dia você esquecer a senha, esta chave permite criar uma senha nova em "Esqueci minha senha". Ela aparece
        só agora; o aplicativo não guarda uma cópia.
      </p>
      <p className="recovery-key mono" aria-label="Chave de recuperação">
        {recoveryKey}
      </p>
      <label>
        Seu e-mail
        <input type="email" value={email} onChange={(e) => setEmail(e.target.value)} placeholder="voce@exemplo.com" />
      </label>
      <div className="actions wrap">
        <button type="button" onClick={sendEmail} disabled={!email.trim()}>
          Enviar a chave para meu e-mail
        </button>
        <button type="button" className="secondary" onClick={copy}>
          Copiar chave
        </button>
      </div>
      <p className="footnote left">
        O aplicativo abre o seu programa de e-mail (Outlook, Mail etc.) com a mensagem pronta para você mesmo; nada é
        enviado sem você clicar em Enviar. Se nenhum programa abrir, use "Copiar chave" e mande para você pelo Gmail ou
        anote num papel.
      </p>
      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}
      <label className="inline">
        <input type="checkbox" checked={saved} onChange={(e) => setSaved(e.target.checked)} />
        Já guardei a chave em local seguro
      </label>
      <button type="button" onClick={onDone} disabled={!saved}>
        {doneLabel}
      </button>
    </div>
  );
}
