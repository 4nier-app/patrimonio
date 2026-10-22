import { useState } from "react";
import { errorMessage, exitCostApi, IOF_PCT, type Dec } from "../api";
import { parseDecimalInput } from "../format";

/** "1,00%" */
export function pctText(v: Dec | number): string {
  return `${Number(v).toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}%`;
}

/** Máscara de calculadora para o percentual: só números, 2 casas ("100" → "1,00"). */
export function pctTyping(raw: string): string {
  const all = raw.replace(/\D/g, "");
  const digits = all.replace(/^0+/, "").slice(0, 4);
  if (!digits) return all ? "0,00" : "";
  const padded = digits.padStart(3, "0");
  return `${padded.slice(0, -2)},${padded.slice(-2)}`;
}

export function totalPct(fee: Dec): number {
  return Number(fee) + Number(IOF_PCT);
}

/** Custo para trazer ao Brasil de uma corretora em dólar: taxa de conversão da corretora + IOF (fixo). */
export default function ExitCostBox({ account, current, onSaved, compact }: { account: string; current: Dec | undefined; onSaved: () => void; compact?: boolean }) {
  const [fee, setFee] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function open() {
    setError(null);
    setFee(current ? Number(current).toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 }) : "");
  }

  async function save(clear = false) {
    if (fee == null) return;
    const value = clear || !fee.trim() ? null : parseDecimalInput(fee);
    if (!clear && fee.trim() && value == null) return setError("Taxa inválida.");
    setBusy(true);
    setError(null);
    try {
      await exitCostApi.set(account, value);
      setFee(null);
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  if (fee == null)
    return (
      <p className="muted small">
        {current && compact ? null : current ? (
          <>
            Custo para trazer ao Brasil: {pctText(totalPct(current))} (conversão {pctText(current)} + IOF {pctText(IOF_PCT)}).{" "}
          </>
        ) : compact ? null : (
          <>Quer ver {account} pelo valor que chegaria no Brasil? </>
        )}
        <button type="button" className="link" onClick={open}>
          {current ? "Alterar" : compact ? "Informar" : "Informar o custo para trazer ao Brasil"}
        </button>
      </p>
    );

  const parsed = fee.trim() ? parseDecimalInput(fee) : null;
  return (
    <div className="savings-move op-form">
      <strong>Custo para trazer ao Brasil · {account}</strong>
      <div className="grid">
        <label>
          Taxa de conversão da corretora (%)
          <input inputMode="numeric" value={fee} placeholder="0,00" autoFocus onChange={(e) => setFee(pctTyping(e.target.value))} />
        </label>
        <label>
          IOF na volta ao Brasil (%)
          <input value={pctText(IOF_PCT).replace("%", "")} disabled />
        </label>
        <label>
          Total descontado
          <input value={parsed != null ? pctText(totalPct(parsed)) : "—"} disabled style={{ fontWeight: 600 }} />
        </label>
      </div>
      <p className="muted small">
        Confira a taxa no app da corretora, na tela de enviar dólares para o Brasil. Com a taxa informada, esta corretora aparece em todo o aplicativo
        pelo <strong>valor de resgate</strong>: dólar comercial menos a taxa e o IOF, o que chegaria na sua conta no Brasil. Em branco, continua pela
        PTAX. <strong>Os impostos sempre usam a PTAX</strong>, como manda a lei.
      </p>
      {error && <p className="error">{error}</p>}
      <div className="actions">
        <button type="button" disabled={busy} onClick={() => void save()}>
          {busy ? "Salvando…" : "Salvar"}
        </button>
        <button type="button" className="secondary" onClick={() => setFee(null)}>
          Cancelar
        </button>
        {current && (
          <button type="button" className="link subtle" disabled={busy} onClick={() => void save(true)}>
            Voltar para a PTAX
          </button>
        )}
      </div>
    </div>
  );
}
