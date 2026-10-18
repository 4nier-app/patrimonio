import { useState } from "react";
import CashInput from "./CashInput";
import { brokerageApi, errorMessage, type Brokerage } from "../api";
import { date, money, parseDecimalInput, toCashMask } from "../format";

const today = () => new Date().toLocaleDateString("sv-SE");

/** Corretagem por ordem de uma conta em reais, para os lançamentos à mão. */
export default function BrokerageBox({ account, current, onSaved }: { account: string; current: Brokerage | undefined; onSaved: () => void }) {
  const [form, setForm] = useState<{ perOrder: string; iss: string; since: string } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function open() {
    setError(null);
    setForm({
      perOrder: current ? toCashMask(current.per_order, 2, "BRL").text : "",
      iss: current ? current.iss_pct.replace(".", ",") : "",
      since: current?.since ?? today(),
    });
  }

  async function save(e: React.FormEvent, zero = false) {
    e.preventDefault();
    if (!form) return;
    const perOrder = zero ? "0" : parseDecimalInput(form.perOrder) ?? "0";
    const iss = zero ? "0" : form.iss.trim() ? parseDecimalInput(form.iss) : "0";
    if (iss == null) return setError("ISS inválido.");
    setBusy(true);
    setError(null);
    try {
      await brokerageApi.set(account, { per_order: perOrder, iss_pct: iss, since: form.since });
      setForm(null);
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  if (!form)
    return (
      <p className="muted small">
        {current ? (
          <>
            Corretagem de {account}: {money(current.per_order)} por ordem
            {Number(current.iss_pct) > 0 && ` + ISS de ${current.iss_pct.replace(".", ",")}%`}, desde {date(current.since)}.{" "}
          </>
        ) : (
          <>{account} cobra corretagem? </>
        )}
        <button type="button" className="link" onClick={open}>
          {current ? "Alterar" : "Informar a corretagem"}
        </button>
      </p>
    );

  return (
    <div className="savings-move op-form">
      <strong>Corretagem de {account}</strong>
      <div className="grid">
        <label>
          Valor por ordem
          <CashInput value={form.perOrder} decimals={2} currency="BRL" onChange={(v) => setForm({ ...form, perOrder: v })} />
        </label>
        <label>
          ISS sobre a corretagem (%)
          <input inputMode="decimal" value={form.iss} placeholder="Ex.: 5" onChange={(e) => setForm({ ...form, iss: e.target.value })} />
        </label>
        <label>
          Vale a partir de
          <input type="date" value={form.since} onChange={(e) => setForm({ ...form, since: e.target.value })} required />
        </label>
      </div>
      <p className="muted small">
        Soma em cada compra ou venda desta conta lançada à mão, a partir da data. Lançamentos de antes da data e notas importadas (que já trazem as
        taxas) não mudam.
      </p>
      {error && <p className="error">{error}</p>}
      <div className="actions">
        <button type="button" disabled={busy} onClick={(e) => void save(e)}>
          {busy ? "Salvando…" : "Salvar"}
        </button>
        <button type="button" className="secondary" onClick={() => setForm(null)}>
          Cancelar
        </button>
        {current && (
          <button type="button" className="link subtle" disabled={busy} onClick={(e) => void save(e, true)}>
            Apagar a corretagem
          </button>
        )}
      </div>
    </div>
  );
}
