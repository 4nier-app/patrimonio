import { Fragment, useEffect, useMemo, useState } from "react";
import {
  errorMessage,
  market,
  type Account,
  type Asset,
  type Currency,
  type Operation,
  type Preview,
  type PricedOperation,
  type Side,
} from "../api";
import {
  cashMask,
  date,
  decimalMask,
  keepNumberChars,
  maskDecimal,
  money,
  parseDecimalInput,
  price,
  priceDecimals,
  quantity,
  toCashMask,
} from "../format";
import { currencyOfNewAccount, normalizeTicker } from "../currency";

type Form = {
  id: string;
  account: string;
  asset: string;
  date: string;
  side: Side;
  quantity: string;
  price: string;
  /** Casas do preço na máscara (2 em reais, 4 em dólar, ou mais se o valor gravado tiver). */
  priceDecimals: number;
  manual: boolean;
  fees: string;
  note: string;
  fromAccount: string;
  residue: string;
};

function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

const EMPTY: Form = {
  id: "",
  account: "",
  asset: "",
  date: todayIso(),
  side: "COMPRA",
  quantity: "",
  price: "",
  priceDecimals: 2,
  manual: false,
  fees: "",
  note: "",
  fromAccount: "",
  residue: "",
};

const SIDE_LABEL: Record<Side, string> = { COMPRA: "Compra", VENDA: "Venda", TRANSFERENCIA: "Transferência" };
const SIDE_BADGE: Record<Side, string> = { COMPRA: "criada", VENDA: "removida", TRANSFERENCIA: "alterada" };

function NumberInput({
  value,
  kind,
  onChange,
  placeholder,
  required,
  usd,
}: {
  value: string;
  kind: "money" | "quantity";
  onChange: (v: string) => void;
  placeholder?: string;
  required?: boolean;
  /** Conta no exterior: ponto vale como decimal, como nas notas. */
  usd?: boolean;
}) {
  return (
    <input
      inputMode="decimal"
      value={value}
      placeholder={placeholder}
      required={required}
      onChange={(e) => onChange(keepNumberChars(e.target.value))}
      onBlur={() => onChange(maskDecimal(value, kind, usd ? "USD" : "BRL", usd))}
    />
  );
}

/** Campo de dinheiro com máscara de caixa: só números, da direita para a esquerda. */
function CashInput({
  value,
  decimals,
  currency,
  onChange,
  required,
}: {
  value: string;
  decimals: number;
  currency: Currency;
  onChange: (v: string) => void;
  required?: boolean;
}) {
  return (
    <input
      inputMode="numeric"
      value={value}
      placeholder={`${currency === "USD" ? "US$" : "R$"} 0,${"0".repeat(decimals)}`}
      required={required}
      onChange={(e) => onChange(cashMask(e.target.value, decimals, currency))}
    />
  );
}

/**
 * Preço digitado: máscara de caixa (só números, preenche da direita), com 2
 * casas em reais e 8 em dólar, como o preço médio das notas da Nomad.
 */
function maskPrice(raw: string, currency: Currency, decimals: number): string {
  return cashMask(raw, decimals, currency);
}

/** Preço colado ("534.7361" ou "534,7361"): ponto ou vírgula valem como decimal. */
function pastedPrice(text: string, currency: Currency): { text: string; decimals: number } | null {
  const dec = parseDecimalInput(decimalMask(text, priceDecimals(currency), currency));
  return dec == null ? null : priceText(dec, currency);
}

/** Valor exato ("95.84") no formato do campo de preço. */
function priceText(dec: string, currency: Currency): { text: string; decimals: number } {
  return toCashMask(dec, priceDecimals(currency), currency);
}

/** Converte o formulário; devolve a mensagem do primeiro campo inválido. */
function toOperation(f: Form, currency: Currency): Operation | string {
  const transfer = f.side === "TRANSFERENCIA";
  const usd = currency === "USD";
  const q = parseDecimalInput(f.quantity, usd);
  if (!q) return "Quantidade inválida.";
  const p = transfer ? "0" : parseDecimalInput(f.price);
  if (!p) return "Preço inválido.";
  let fees: string | null = null;
  if (f.manual && !transfer) {
    fees = parseDecimalInput(f.fees);
    if (fees == null) return "Taxa inválida.";
  }
  let residue: string | null = null;
  if (transfer && f.residue.trim()) {
    residue = parseDecimalInput(f.residue, usd);
    if (residue == null) return "Resíduo inválido.";
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
    currency,
    from_account: transfer ? f.fromAccount.trim() || null : null,
    residue,
  };
}

export default function Operations() {
  const [list, setList] = useState<PricedOperation[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
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

  const existing = accounts.find((a) => a.name === form.account.trim());
  // Conta existente: a moeda dela. Conta nova: pelo nome da corretora
  // (Nomad → dólar) ou, se o nome não disser, pelo ativo.
  const currency: Currency = existing?.currency ?? currencyOfNewAccount(form.account, form.asset, suggestions);
  const transfer = form.side === "TRANSFERENCIA";
  const draft = useMemo(() => toOperation(form, currency), [form, currency]);

  // Ao trocar a moeda, o preço passa a ter as casas da nova moeda.
  useEffect(() => {
    setForm((f) => {
      if (f.id) return f;
      const decimals = priceDecimals(currency);
      if (f.priceDecimals === decimals && !f.price.startsWith(currency === "USD" ? "R$" : "US$")) return f;
      // Mantém o valor digitado, só troca o símbolo e as casas.
      const dec = (v: string) => parseDecimalInput(v);
      const price = dec(f.price);
      const fees = dec(f.fees);
      return {
        ...f,
        priceDecimals: decimals,
        price: price == null ? "" : priceText(price, currency).text,
        fees: fees == null ? "" : toCashMask(fees, 2, currency).text,
      };
    });
  }, [currency]);

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
      setForm({ ...EMPTY, account: form.account, date: form.date, priceDecimals: priceDecimals(currency) });
      setReason("");
      load();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function edit(p: PricedOperation) {
    const o = p.operation;
    const cur = o.currency;
    const pr = o.side === "TRANSFERENCIA" ? { text: "", decimals: priceDecimals(cur) } : priceText(o.price, cur);
    setForm({
      id: o.id,
      account: o.account,
      asset: o.asset,
      date: o.date,
      side: o.side,
      quantity: maskDecimal(o.quantity, "quantity", cur, cur === "USD"),
      price: pr.text,
      priceDecimals: pr.decimals,
      manual: o.manual_fees != null,
      fees: o.manual_fees != null ? toCashMask(o.manual_fees, 2, cur).text : "",
      note: o.note ?? "",
      fromAccount: o.from_account ?? "",
      residue: o.residue ? maskDecimal(o.residue, "quantity", cur, cur === "USD") : "",
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
    preview && !preview.valid && !transfer
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
            Compras e vendas na B3 e no exterior. Na B3, as taxas (liquidação, emolumentos e transferência) são
            calculadas sobre o total da nota, ou seja, de todos os lançamentos da mesma corretora no mesmo dia, e
            divididas entre eles. Se a nota trouxer outro valor, informe a taxa à mão. No exterior os valores são em
            dólar e a conversão para reais usa a PTAX do Banco Central.
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
                <option key={a.name} value={a.name}>
                  {a.currency === "USD" ? "Dólar (exterior)" : "Reais (B3)"}
                </option>
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
              <option value="TRANSFERENCIA">Transferência de custódia</option>
            </select>
          </label>
          {transfer && (
            <label>
              Corretora de origem
              <select value={form.fromAccount} onChange={(e) => set("fromAccount", e.target.value)} required>
                <option value="">Escolha a conta de onde as ações saíram</option>
                {accounts
                  .filter((a) => a.currency === currency && a.name !== form.account.trim())
                  .map((a) => (
                    <option key={a.name} value={a.name}>
                      {a.name}
                    </option>
                  ))}
              </select>
            </label>
          )}
          <label>
            Ativo (código)
            <input
              list="ativos"
              value={form.asset}
              onChange={(e) => set("asset", e.target.value.toUpperCase())}
              onBlur={(e) => set("asset", normalizeTicker(e.target.value))}
              placeholder={currency === "USD" ? "Ex.: VOO" : "Ex.: ABCD3"}
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
            {transfer ? "Quantidade transferida" : "Quantidade"}
            <NumberInput kind="quantity" value={form.quantity} onChange={(v) => set("quantity", v)} usd={currency === "USD"} required />
          </label>
          {transfer ? (
            <label>
              Resíduo que ficou na origem (opcional)
              <NumberInput kind="quantity" value={form.residue} onChange={(v) => set("residue", v)} usd={currency === "USD"} placeholder="Ex.: 0,0000012" />
            </label>
          ) : (
            <label>
              Preço unitário
              <input
                inputMode="numeric"
                value={form.price}
                placeholder={currency === "USD" ? "US$ 0,00000000" : "R$ 0,00"}
                required
                onChange={(e) => set("price", maskPrice(e.target.value, currency, form.priceDecimals))}
                onPaste={(e) => {
                  const p = pastedPrice(e.clipboardData.getData("text"), currency);
                  if (!p) return;
                  e.preventDefault();
                  setForm((f) => ({ ...f, price: p.text, priceDecimals: p.decimals }));
                }}
              />
              {currency === "USD" && (
                <span className="muted small">Digite só os números, com 8 casas: 53473610000 = US$ 534,73610000.</span>
              )}
            </label>
          )}
        </div>

        {transfer && (
          <p className="muted small">
            A posição muda de corretora sem compra nem venda: o preço médio e as datas de compra continuam os mesmos. Se
            uma fração ficou para trás e não veio para a nova corretora, informe-a como resíduo: ela sai da carteira sem
            gerar lucro nem prejuízo de venda.
          </p>
        )}

        {!transfer && (
          <label className="inline">
            <input type="checkbox" checked={form.manual} onChange={(e) => set("manual", e.target.checked)} />
            {currency === "USD" ? "Informar taxas da corretora" : "Informar as taxas à mão (substitui o cálculo automático)"}
          </label>
        )}
        {form.manual && !transfer && (
          <label className="short">
            Taxas deste lançamento
            <CashInput value={form.fees} decimals={2} currency={currency} onChange={(v) => set("fees", v)} />
          </label>
        )}

        <label>
          Observação (opcional)
          <input value={form.note} onChange={(e) => set("note", e.target.value)} />
        </label>

        {preview && !preview.valid && !transfer && (
          <div className="preview">
            <div>
              Valor da operação: <strong>{money(preview.gross, currency)}</strong> · Taxas ({preview.fee_origin}):{" "}
              <strong>{money(preview.fees, currency)}</strong> · {form.side === "COMPRA" ? "Total pago" : "Total recebido"}:{" "}
              <strong>{money(total == null ? null : String(total), currency)}</strong>
            </div>
            {currency === "USD" && (
              <p className="muted small">No exterior não há taxas da B3. A conversão para reais usa a PTAX do dia da operação.</p>
            )}
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
                  <td>
                    {p.operation.account}
                    {p.operation.from_account && <div className="muted small">vindo de {p.operation.from_account}</div>}
                  </td>
                  <td>
                    <strong>{p.operation.asset}</strong>
                    {p.operation.note && <div className="muted small">{p.operation.note}</div>}
                  </td>
                  <td>
                    <span className={`badge ${SIDE_BADGE[p.operation.side]}`}>{SIDE_LABEL[p.operation.side]}</span>
                  </td>
                  <td className="num">
                    {quantity(p.operation.quantity)}
                    {p.operation.residue && Number(p.operation.residue) > 0 && (
                      <div className="muted small">resíduo {quantity(p.operation.residue)}</div>
                    )}
                  </td>
                  <td className="num">{p.operation.side === "TRANSFERENCIA" ? "—" : price(p.operation.price, p.operation.currency)}</td>
                  <td className="num">{p.operation.side === "TRANSFERENCIA" ? "—" : money(p.gross, p.operation.currency)}</td>
                  <td className="num">
                    {money(p.fees, p.operation.currency)}
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
