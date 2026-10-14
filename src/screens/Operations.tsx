import { Fragment, useEffect, useMemo, useState } from "react";
import { errorMessage, market, type Asset, type Operation, type Preview, type PricedOperation, type Side } from "../api";
import { date, keepNumberChars, maskDecimal, money, parseDecimalInput, price, quantity } from "../format";

type Form = {
  id: string;
  account: string;
  asset: string;
  date: string;
  side: Side;
  quantity: string;
  price: string;
  manual: boolean;
  fees: string;
  note: string;
};

function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

const EMPTY: Form = { id: "", account: "", asset: "", date: todayIso(), side: "COMPRA", quantity: "", price: "", manual: false, fees: "", note: "" };

function NumberInput({
  value,
  kind,
  onChange,
  placeholder,
  required,
}: {
  value: string;
  kind: "money" | "quantity";
  onChange: (v: string) => void;
  placeholder?: string;
  required?: boolean;
}) {
  return (
    <input
      inputMode="decimal"
      value={value}
      placeholder={placeholder}
      required={required}
      onChange={(e) => onChange(keepNumberChars(e.target.value))}
      onBlur={() => onChange(maskDecimal(value, kind))}
    />
  );
}

/** Converte o formulário; devolve a mensagem do primeiro campo inválido. */
function toOperation(f: Form): Operation | string {
  const q = parseDecimalInput(f.quantity);
  if (!q) return "Quantidade inválida.";
  const p = parseDecimalInput(f.price);
  if (!p) return "Preço inválido.";
  let fees: string | null = null;
  if (f.manual) {
    fees = parseDecimalInput(f.fees);
    if (fees == null) return "Taxa inválida.";
  }
  if (!f.date) return "Informe a data.";
  return {
    id: f.id,
    account: f.account,
    asset: f.asset,
    date: f.date,
    side: f.side,
    quantity: q,
    price: p,
    manual_fees: fees,
    note: f.note.trim() || null,
  };
}

export default function Operations() {
  const [list, setList] = useState<PricedOperation[]>([]);
  const [accounts, setAccounts] = useState<string[]>([]);
  const [suggestions, setSuggestions] = useState<Asset[]>([]);
  const [form, setForm] = useState<Form>(EMPTY);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [reason, setReason] = useState("");
  const [removing, setRemoving] = useState<string | null>(null);
  const [removeReason, setRemoveReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  function load() {
    market.listOperations().then(setList).catch((e) => setError(errorMessage(e)));
    market.listAccounts().then(setAccounts).catch(() => {});
  }
  useEffect(load, []);

  // Sugestões do catálogo enquanto digita o código.
  useEffect(() => {
    if (form.asset.trim().length < 2) {
      setSuggestions([]);
      return;
    }
    const t = setTimeout(() => market.searchAssets(form.asset).then(setSuggestions).catch(() => {}), 150);
    return () => clearTimeout(t);
  }, [form.asset]);

  const draft = useMemo(() => toOperation(form), [form]);

  // Taxas calculadas ao vivo, já considerando os outros lançamentos da mesma nota.
  useEffect(() => {
    if (typeof draft === "string" || !form.asset || !form.account) {
      setPreview(null);
      return;
    }
    const t = setTimeout(() => market.preview(draft).then(setPreview).catch(() => setPreview(null)), 200);
    return () => clearTimeout(t);
  }, [draft, form.asset, form.account]);

  function set<K extends keyof Form>(key: K, value: Form[K]) {
    setForm((f) => ({ ...f, [key]: value }));
  }

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setMessage(null);
    if (typeof draft === "string") {
      setError(draft);
      return;
    }
    try {
      if (form.id) {
        await market.update(draft, reason);
        setMessage("Lançamento alterado. A alteração ficou registrada na Auditoria.");
      } else {
        await market.create(draft);
        setMessage("Lançamento incluído.");
      }
      setForm({ ...EMPTY, account: form.account, date: form.date });
      setReason("");
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function edit(p: PricedOperation) {
    const o = p.operation;
    setForm({
      id: o.id,
      account: o.account,
      asset: o.asset,
      date: o.date,
      side: o.side,
      quantity: maskDecimal(o.quantity, "quantity"),
      price: maskDecimal(o.price, "money"),
      manual: o.manual_fees != null,
      fees: o.manual_fees != null ? maskDecimal(o.manual_fees, "money") : "",
      note: o.note ?? "",
    });
    setReason("");
    setMessage(null);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  async function confirmRemove(id: string) {
    setError(null);
    try {
      await market.remove(id, removeReason);
      setRemoving(null);
      setRemoveReason("");
      setMessage("Lançamento removido. Ele continua visível na Auditoria.");
      if (form.id === id) setForm(EMPTY);
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const total =
    preview && !preview.valid
      ? form.side === "COMPRA"
        ? Number(preview.gross) + Number(preview.fees)
        : Number(preview.gross) - Number(preview.fees)
      : null;

  return (
    <section>
      <div className="section-head">
        <div>
          <h2>Lançamentos</h2>
          <p className="muted">
            Compras e vendas na B3. As taxas da B3 (liquidação, emolumentos e transferência) são calculadas sobre o total
            da nota, ou seja, de todos os lançamentos da mesma corretora no mesmo dia, e divididas entre eles. Se a nota
            trouxer outro valor, informe a taxa à mão.
          </p>
        </div>
      </div>

      <form className="card op-form" onSubmit={save}>
        <h3>{form.id ? "Alterar lançamento" : "Novo lançamento"}</h3>
        <div className="grid">
          <label>
            Corretora (conta)
            <input list="contas" value={form.account} onChange={(e) => set("account", e.target.value)} placeholder="Ex.: Itaú" required />
            <datalist id="contas">
              {accounts.map((a) => (
                <option key={a} value={a} />
              ))}
            </datalist>
          </label>
          <label>
            Data do pregão
            <input type="date" value={form.date} onChange={(e) => set("date", e.target.value)} required />
          </label>
          <label>
            Operação
            <select value={form.side} onChange={(e) => set("side", e.target.value as Side)}>
              <option value="COMPRA">Compra</option>
              <option value="VENDA">Venda</option>
            </select>
          </label>
          <label>
            Ativo (código)
            <input
              list="ativos"
              value={form.asset}
              onChange={(e) => set("asset", e.target.value.toUpperCase())}
              placeholder="Ex.: ABCD3"
              required
            />
            <datalist id="ativos">
              {suggestions.map((a) => (
                <option key={a.ticker} value={a.ticker}>
                  {a.name} · {a.kind}
                </option>
              ))}
            </datalist>
          </label>
          <label>
            Quantidade
            <NumberInput kind="quantity" value={form.quantity} onChange={(v) => set("quantity", v)} required />
          </label>
          <label>
            Preço unitário
            <NumberInput kind="money" value={form.price} onChange={(v) => set("price", v)} placeholder="R$ 0,00" required />
          </label>
        </div>

        <label className="inline">
          <input type="checkbox" checked={form.manual} onChange={(e) => set("manual", e.target.checked)} />
          Informar as taxas à mão (substitui o cálculo automático)
        </label>
        {form.manual && (
          <label className="short">
            Taxas deste lançamento
            <NumberInput kind="money" value={form.fees} onChange={(v) => set("fees", v)} placeholder="R$ 0,00" />
          </label>
        )}

        <label>
          Observação (opcional)
          <input value={form.note} onChange={(e) => set("note", e.target.value)} />
        </label>

        {preview && !preview.valid && (
          <div className="preview">
            <div>
              Valor da operação: <strong>{money(preview.gross)}</strong> · Taxas ({preview.fee_origin}):{" "}
              <strong>{money(preview.fees)}</strong> · {form.side === "COMPRA" ? "Total pago" : "Total recebido"}:{" "}
              <strong>{money(total == null ? null : String(total))}</strong>
            </div>
            {preview.fee_origin === "automática" && preview.note_fees.length > 0 && (
              <details>
                <summary>
                  Como as taxas foram calculadas (nota com {preview.note_operations} lançamento
                  {preview.note_operations > 1 ? "s" : ""})
                </summary>
                <ul className="mono">
                  {preview.note_fees.map((f) => (
                    <li key={f.kind}>
                      {f.kind}: {f.formula}
                    </li>
                  ))}
                </ul>
                {preview.note_operations > 1 && (
                  <p className="muted small">Cada taxa da nota é dividida entre os lançamentos na proporção do valor de cada um.</p>
                )}
              </details>
            )}
          </div>
        )}

        {form.id && (
          <label>
            Motivo da alteração (fica na Auditoria)
            <input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Ex.: preço digitado errado" />
          </label>
        )}

        {error && <p className="error">{error}</p>}
        {message && <p className="ok">{message}</p>}

        <div className="actions">
          <button type="submit">{form.id ? "Salvar alteração" : "Incluir lançamento"}</button>
          {form.id && (
            <button type="button" className="secondary" onClick={() => setForm(EMPTY)}>
              Cancelar
            </button>
          )}
        </div>
      </form>

      <div className="scroll spaced">
        <table>
          <thead>
            <tr>
              <th>Data</th>
              <th>Corretora</th>
              <th>Ativo</th>
              <th>Operação</th>
              <th className="num">Qtd.</th>
              <th className="num">Preço</th>
              <th className="num">Valor</th>
              <th className="num">Taxas</th>
              <th></th>
            </tr>
          </thead>
          <tbody>
            {list.length === 0 && (
              <tr>
                <td colSpan={9} className="muted">
                  Nenhum lançamento.
                </td>
              </tr>
            )}
            {list.map((p) => (
              <Fragment key={p.operation.id}>
                <tr>
                  <td>{date(p.operation.date)}</td>
                  <td>{p.operation.account}</td>
                  <td>
                    <strong>{p.operation.asset}</strong>
                    {p.operation.note && <div className="muted small">{p.operation.note}</div>}
                  </td>
                  <td>
                    <span className={`badge ${p.operation.side === "COMPRA" ? "criada" : "removida"}`}>
                      {p.operation.side === "COMPRA" ? "Compra" : "Venda"}
                    </span>
                  </td>
                  <td className="num">{quantity(p.operation.quantity)}</td>
                  <td className="num">{price(p.operation.price)}</td>
                  <td className="num">{money(p.gross)}</td>
                  <td className="num">
                    {money(p.fees)}
                    {p.fee_origin === "informada" && <div className="muted small">informada</div>}
                  </td>
                  <td className="row-actions">
                    <button className="link" onClick={() => edit(p)}>
                      Alterar
                    </button>
                    <button className="link danger" onClick={() => setRemoving(p.operation.id)}>
                      Remover
                    </button>
                  </td>
                </tr>
                {removing === p.operation.id && (
                  <tr className="detail">
                    <td colSpan={9}>
                      <div className="remove-box">
                        <input
                          autoFocus
                          value={removeReason}
                          onChange={(e) => setRemoveReason(e.target.value)}
                          placeholder="Motivo da remoção (obrigatório)"
                        />
                        <button onClick={() => confirmRemove(p.operation.id)} disabled={!removeReason.trim()}>
                          Confirmar remoção
                        </button>
                        <button className="secondary" onClick={() => setRemoving(null)}>
                          Cancelar
                        </button>
                      </div>
                    </td>
                  </tr>
                )}
              </Fragment>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
