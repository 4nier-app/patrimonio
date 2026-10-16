import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import Intro from "../components/Intro";
import { offerUndo } from "../undo";
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
import { currencyOfNewAccount, fundTicker, isFund, normalizeTicker } from "../currency";
import CashInput from "../components/CashInput";

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
  /** Fundo de investimento: lançado pelo extrato do banco, em cotas. */
  fund: boolean;
  /** Fundo: come-cotas (resgate semestral de cotas para pagar o IR). */
  comeCotas: boolean;
  ir: string;
  iof: string;
  /** Fundo fora do catálogo da CVM: nome digitado. */
  fundName: string;
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
  fund: false,
  comeCotas: false,
  ir: "",
  iof: "",
  fundName: "",
};

const COME_COTAS = "Come-cotas";


const MONTH_NAMES = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

const SIDE_LABEL: Record<Side, string> = { COMPRA: "Compra", VENDA: "Venda", TRANSFERENCIA: "Transferência", AJUSTE: "Evento" };
const FUND_SIDE_LABEL: Record<Side, string> = { COMPRA: "Aplicação", VENDA: "Resgate", TRANSFERENCIA: "Transferência", AJUSTE: "Evento" };
const SIDE_BADGE: Record<Side, string> = { COMPRA: "criada", VENDA: "removida", TRANSFERENCIA: "alterada", AJUSTE: "alterada" };

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
  if (!p) return f.fund ? "Valor da cota inválido." : "Preço inválido.";
  let ir: string | null = null;
  let iof: string | null = null;
  if (f.fund && f.side === "VENDA") {
    // Come-cotas: o núcleo põe todo o valor das cotas como IR.
    if (!f.comeCotas) {
      if (f.ir.trim()) {
        ir = parseDecimalInput(f.ir);
        if (ir == null) return "IR inválido.";
      }
      if (f.iof.trim()) {
        iof = parseDecimalInput(f.iof);
        if (iof == null) return "IOF inválido.";
      }
    }
  }
  let fees: string | null = null;
  if (f.manual && !transfer && !f.fund) {
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
    asset: f.fund ? fundTicker(f.asset) ?? f.asset : f.asset,
    date: f.date,
    side: f.side,
    quantity: q,
    price: p,
    manual_fees: fees,
    note: f.fund && f.side === "VENDA" && f.comeCotas ? COME_COTAS : f.note.trim() || null,
    currency,
    from_account: transfer ? f.fromAccount.trim() || null : null,
    residue,
    ir,
    iof,
  };
}

/** `onChanged`: chamado depois de incluir, alterar ou remover, para atualizar as cotações. */
export default function Operations({ onChanged }: { onChanged: () => void }) {
  const [list, setList] = useState<PricedOperation[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [suggestions, setSuggestions] = useState<Asset[]>([]);
  const [form, setForm] = useState<Form>(EMPTY);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [reason, setReason] = useState("");
  const [removing, setRemoving] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);

  function load() {
    market.listOperations().then(setList).catch((e) => setError(errorMessage(e)));
    market.listAccounts().then(setAccounts).catch(() => {});
  }
  useEffect(load, []);

  // Nome dos fundos da lista (o código deles é o CNPJ).
  const [names, setNames] = useState<Record<string, string>>({});
  useEffect(() => {
    const funds = [...new Set(list.map((p) => p.operation.asset).filter(isFund))].filter((t) => !(t in names));
    if (!funds.length) return;
    Promise.all(funds.map((t) => market.searchFunds(t).then((r) => [t, r[0]?.name ?? ""] as const).catch(() => [t, ""] as const))).then(
      (pairs) => setNames((n) => ({ ...n, ...Object.fromEntries(pairs) })),
    );
  }, [list]);

  // Lista de fundos da CVM: baixada ao lançar o primeiro fundo, renovada a cada 30 dias.
  const [catalog, setCatalog] = useState<string | null>(null);
  const [catalogReady, setCatalogReady] = useState(0);
  const catalogAsked = useRef(false);
  useEffect(() => {
    if (!form.fund || catalogAsked.current) return;
    catalogAsked.current = true;
    setCatalog("Baixando a lista de fundos da CVM (só na primeira vez; pode levar um minuto)…");
    market
      .updateFundCatalog()
      .then((line) => {
        setCatalog(line.ok ? null : `Não consegui baixar a lista de fundos da CVM: ${line.message}. Você ainda pode lançar pelo CNPJ.`);
        setCatalogReady((n) => n + 1);
      })
      .catch((e) => setCatalog(`Não consegui baixar a lista de fundos da CVM: ${errorMessage(e)}. Você ainda pode lançar pelo CNPJ.`));
  }, [form.fund]);

  // Sugestões do catálogo enquanto digita o código (ou o nome do fundo).
  useEffect(() => {
    if (form.asset.trim().length < 2) {
      setSuggestions([]);
      return;
    }
    const search = form.fund ? market.searchFunds : market.searchAssets;
    const t = setTimeout(() => search(form.asset).then(setSuggestions).catch(() => {}), 150);
    return () => clearTimeout(t);
  }, [form.asset, form.fund, catalogReady]);


  const existing = accounts.find((a) => a.name === form.account.trim());
  // Conta existente: a moeda dela. Conta nova: pelo nome da corretora
  // (Nomad → dólar) ou, se o nome não disser, pelo ativo.
  const currency: Currency = form.fund ? "BRL" : existing?.currency ?? currencyOfNewAccount(form.account, form.asset, suggestions);
  const fundCode = form.fund ? fundTicker(form.asset) : null;
  const fundInfo = fundCode ? suggestions.find((a) => a.ticker === fundCode) : undefined;
  const [knownFund, setKnownFund] = useState<Asset | null>(null);
  // Fundo escolhido: o nome vem do catálogo (ou do que foi digitado antes).
  useEffect(() => {
    if (!fundCode) {
      setKnownFund(null);
      return;
    }
    if (fundInfo) {
      setKnownFund(fundInfo);
      return;
    }
    market
      .searchFunds(fundCode)
      .then((r) => setKnownFund(r[0] ?? null))
      .catch(() => setKnownFund(null));
  }, [fundCode, fundInfo, catalogReady]);
  const fundNamed = knownFund != null && knownFund.name.trim() !== "";
  const transfer = form.side === "TRANSFERENCIA";
  const coin = suggestions.find((a) => a.ticker === normalizeTicker(form.asset) && a.kind === "CRIPTO");
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
      if (form.fund && fundCode && !fundNamed && form.fundName.trim()) {
        await market.nameFund(fundCode, form.fundName);
      }
      if (form.id) {
        await market.update(draft, reason);
        setMessage("Lançamento alterado. A alteração ficou registrada na Auditoria.");
      } else {
        await market.create(draft);
        setMessage("Lançamento incluído.");
      }
      const next: Form = { ...EMPTY, account: form.account, date: form.date, fund: form.fund, priceDecimals: priceDecimals(currency) };
      // Na inclusão, a próxima já vem com o mesmo ativo e operação (notas e
      // extratos costumam ter várias linhas iguais); só os valores são limpos.
      if (!form.id) Object.assign(next, { asset: form.asset, side: form.side, comeCotas: form.comeCotas, manual: form.manual, fromAccount: form.fromAccount });
      setForm(next);
      setReason("");
      load();
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function edit(p: PricedOperation) {
    const o = p.operation;
    const cur = o.currency;
    const fund = isFund(o.asset);
    const pr =
      o.side === "TRANSFERENCIA"
        ? { text: "", decimals: priceDecimals(cur) }
        : fund
          ? { text: decimalMask(o.price.replace(".", ","), 10, "BRL"), decimals: 2 }
          : priceText(o.price, cur);
    const comeCotas = fund && o.side === "VENDA" && o.note === COME_COTAS;
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
      note: comeCotas ? "" : o.note ?? "",
      fromAccount: o.from_account ?? "",
      residue: o.residue ? maskDecimal(o.residue, "quantity", cur, cur === "USD") : "",
      fund,
      comeCotas,
      ir: o.ir && !comeCotas ? toCashMask(o.ir, 2, cur).text : "",
      iof: o.iof ? toCashMask(o.iof, 2, cur).text : "",
      fundName: "",
    });
    setReason("");
    setMessage(null);
    window.scrollTo({ top: 0, behavior: "smooth" });
  }

  async function confirmRemove(id: string) {
    setError(null);
    try {
      await market.remove(id);
      setRemoving(null);
      setMessage(null);
      offerUndo("Lançamento removido.", async () => {
        await market.restore([id]);
        load();
        onChanged();
      });
      if (form.id === id) setForm(EMPTY);
      load();
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  /** Mesma nota: mesma conta e mesmo dia. */
  const noteOf = (o: Operation) => list.filter((x) => x.operation.account === o.account && x.operation.date === o.date);

  async function confirmRemoveNote(o: Operation) {
    setError(null);
    const ids = noteOf(o).map((x) => x.operation.id);
    try {
      const n = await market.removeMany(ids);
      setRemoving(null);
      setMessage(null);
      offerUndo(`${n} lançamentos removidos.`, async () => {
        await market.restore(ids);
        load();
        onChanged();
      });
      if (ids.includes(form.id)) setForm(EMPTY);
      load();
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  // Filtros da lista: ano (o mais recente ao abrir), mês, conta e ativo.
  const years = useMemo(() => [...new Set(list.map((p) => Number(p.operation.date.slice(0, 4))))].sort(), [list]);
  const [fYear, setFYear] = useState<number | "todos" | null>(null);
  const [fMonth, setFMonth] = useState(0);
  const [fAccount, setFAccount] = useState("");
  const [fAsset, setFAsset] = useState("");
  const [openNotes, setOpenNotes] = useState<Set<string>>(new Set());
  useEffect(() => {
    if (fYear == null && years.length) setFYear(years[years.length - 1]);
  }, [years, fYear]);
  const shown = list.filter((p) => {
    const o = p.operation;
    if (fYear != null && fYear !== "todos" && Number(o.date.slice(0, 4)) !== fYear) return false;
    if (fMonth && Number(o.date.slice(5, 7)) !== fMonth) return false;
    if (fAccount && o.account !== fAccount) return false;
    const q = fAsset.trim().toUpperCase();
    return !q || o.asset.includes(q) || (names[o.asset] ?? "").includes(q);
  });
  // Nota: mesma conta e mesmo dia, na ordem da lista (mais recente primeiro).
  const groups: { key: string; items: PricedOperation[] }[] = [];
  for (const p of shown) {
    const key = `${p.operation.date}|${p.operation.account}`;
    const g = groups.find((x) => x.key === key);
    if (g) g.items.push(p);
    else groups.push({ key, items: [p] });
  }
  function toggleNote(key: string) {
    setOpenNotes((s) => {
      const n = new Set(s);
      if (n.has(key)) n.delete(key);
      else n.add(key);
      return n;
    });
  }
  const row = (p: PricedOperation) => (
      <Fragment key={p.operation.id}>
        <tr>
          <td>{date(p.operation.date)}</td>
          <td>
            {p.operation.account}
            {p.operation.from_account && <div className="muted small">vindo de {p.operation.from_account}</div>}
          </td>
          <td>
            <strong>{isFund(p.operation.asset) ? names[p.operation.asset] || p.operation.asset : p.operation.asset}</strong>
            {isFund(p.operation.asset) && names[p.operation.asset] && <div className="muted small">{p.operation.asset}</div>}
            {p.operation.note && p.operation.note !== COME_COTAS && <div className="muted small">{p.operation.note}</div>}
          </td>
          <td>
            <span className={`badge ${SIDE_BADGE[p.operation.side]}`}>
              {isFund(p.operation.asset)
                ? p.operation.note === COME_COTAS
                  ? COME_COTAS
                  : FUND_SIDE_LABEL[p.operation.side]
                : SIDE_LABEL[p.operation.side]}
            </span>
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
            {p.fee_origin === "IR/IOF" && Number(p.fees) > 0 && <div className="muted small">IR/IOF</div>}
          </td>
          <td className="row-actions">
            <button className="link" onClick={() => edit(p)}>
              Alterar
            </button>
            <button
              className="link subtle"
              onClick={() => (noteOf(p.operation).length > 1 ? setRemoving(p.operation.id) : void confirmRemove(p.operation.id))}
            >
              Remover
            </button>
          </td>
        </tr>
        {removing === p.operation.id && (
          <tr className="detail">
            <td colSpan={9}>
              <div className="remove-box">
                <span>Este dia tem {noteOf(p.operation).length} lançamentos nesta conta.</span>
                <button autoFocus onClick={() => confirmRemove(p.operation.id)}>
                  Remover só este
                </button>
                {noteOf(p.operation).length > 1 && (
                  <button className="danger" onClick={() => confirmRemoveNote(p.operation)}>
                    Remover todos do dia ({noteOf(p.operation).length} lançamentos de {date(p.operation.date)} em{" "}
                    {p.operation.account})
                  </button>
                )}
                <button className="secondary" onClick={() => setRemoving(null)}>
                  Cancelar
                </button>
              </div>
            </td>
          </tr>
        )}
      </Fragment>
  );

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
          <Intro
            more={
              <>
                Na B3, as taxas (liquidação, emolumentos e transferência) são calculadas sobre o total da nota, ou seja, de todos os lançamentos da
                mesma corretora no mesmo dia, e divididas entre eles. Se a nota trouxer outro valor, informe a taxa à mão. No exterior os valores são em
                dólar e a conversão para reais usa a PTAX do Banco Central. Fundos de investimento são lançados pelo extrato do banco, em cotas.
              </>
            }
          >
            Compras e vendas na B3, no exterior, em cripto e em fundos. As taxas da B3 são calculadas sozinhas.
          </Intro>
        </div>
      </div>

      <form className="card op-form" onSubmit={save}>
        <h3>{form.id ? "Alterar lançamento" : "Novo lançamento"}</h3>
        {!form.id && (
          <div className="op-kind" role="tablist">
            <button
              type="button"
              className={form.fund ? "secondary" : ""}
              onClick={() => setForm((f) => ({ ...EMPTY, account: f.account, date: f.date }))}
            >
              Ações, FIIs, ETFs, exterior ou cripto
            </button>
            <button
              type="button"
              className={form.fund ? "" : "secondary"}
              onClick={() => setForm((f) => ({ ...EMPTY, account: f.account, date: f.date, fund: true }))}
            >
              Fundo de investimento
            </button>
          </div>
        )}
        {form.fund && catalog && <p className="muted small">{catalog}</p>}
        <div className="grid">
          <label>
            {form.fund ? "Banco ou corretora (conta)" : "Corretora (conta)"}
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
            {form.fund ? "Data" : "Data do pregão"}
            <input type="date" value={form.date} onChange={(e) => set("date", e.target.value)} required />
          </label>
          {form.fund ? (
            <label>
              Operação
              <select
                value={form.comeCotas ? "COME" : form.side}
                onChange={(e) =>
                  setForm((f) => ({
                    ...f,
                    side: e.target.value === "COMPRA" ? "COMPRA" : "VENDA",
                    comeCotas: e.target.value === "COME",
                  }))
                }
              >
                <option value="COMPRA">Aplicação</option>
                <option value="VENDA">Resgate</option>
                <option value="COME">Come-cotas (IR semestral)</option>
              </select>
            </label>
          ) : (
            <label>
              Operação
              <select value={form.side} onChange={(e) => set("side", e.target.value as Side)}>
                <option value="COMPRA">Compra</option>
                <option value="VENDA">Venda</option>
                <option value="TRANSFERENCIA">Transferência de custódia</option>
              </select>
            </label>
          )}
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
          {form.fund ? (
            <label className="wide">
              Fundo (nome como no extrato, ou CNPJ)
              <input
                list="fundos"
                value={form.asset}
                onChange={(e) => set("asset", e.target.value.toUpperCase())}
                onBlur={(e) => set("asset", fundTicker(e.target.value) ?? e.target.value.trim().toUpperCase())}
                placeholder="Ex.: RENDA FIXA DI ou 11.222.333/0001-81"
                required
              />
              <datalist id="fundos">
                {suggestions.map((a) => (
                  <option key={a.ticker} value={a.ticker}>
                    {a.name}
                  </option>
                ))}
              </datalist>
              {fundCode && fundNamed && <span className="muted small">{knownFund?.name}</span>}
              {form.asset.trim() !== "" && !fundCode && (
                <span className="muted small">Escolha o fundo na lista ou digite o CNPJ completo (14 números).</span>
              )}
            </label>
          ) : (
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
          )}
          {form.fund && fundCode && !fundNamed && (
            <label>
              Nome do fundo
              <input value={form.fundName} onChange={(e) => set("fundName", e.target.value)} placeholder="Como aparece no extrato" />
              <span className="muted small">Este CNPJ não está na lista da CVM baixada; o nome ajuda a reconhecê-lo na Carteira.</span>
            </label>
          )}
          <label>
            {transfer ? "Quantidade transferida" : form.fund ? "Quantidade de cotas" : "Quantidade"}
            <NumberInput kind="quantity" value={form.quantity} onChange={(v) => set("quantity", v)} usd={currency === "USD"} required />
          </label>
          {transfer ? (
            <label>
              Resíduo que ficou na origem (opcional)
              <NumberInput kind="quantity" value={form.residue} onChange={(v) => set("residue", v)} usd={currency === "USD"} placeholder="Ex.: 0,0000012" />
            </label>
          ) : form.fund ? (
            <label>
              Valor da cota
              <input
                inputMode="decimal"
                value={form.price}
                placeholder="R$ 0,0000000"
                required
                onChange={(e) => set("price", decimalMask(e.target.value, 10, "BRL"))}
              />
              <span className="muted small">Digite como no extrato, com a vírgula: 12,3456789.</span>
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

        {coin && (
          <p className="muted small">
            {coin.name} é criptomoeda: lance numa conta em dólar (por exemplo, "Carteira cripto"), com a quantidade como está
            na carteira (ex.: 0,00123456) e o preço de cada unidade em dólar. A conversão para reais usa a PTAX do dia, como
            no exterior. Não há taxas da B3; se a corretora cobrou taxa, informe abaixo. A cotação é atualizada sozinha.
          </p>
        )}

        {transfer && (
          <p className="muted small">
            A posição muda de corretora sem compra nem venda: o preço médio e as datas de compra continuam os mesmos. Se
            uma fração ficou para trás e não veio para a nova corretora, informe-a como resíduo: ela sai da carteira sem
            gerar lucro nem prejuízo de venda.
          </p>
        )}

        {form.fund && form.side === "VENDA" && !form.comeCotas && (
          <div className="grid">
            <label>
              IR descontado
              <CashInput value={form.ir} decimals={2} currency="BRL" onChange={(v) => set("ir", v)} />
            </label>
            <label>
              IOF descontado
              <CashInput value={form.iof} decimals={2} currency="BRL" onChange={(v) => set("iof", v)} />
            </label>
          </div>
        )}

        {form.fund && (
          <p className="muted small">
            {form.comeCotas
              ? "Come-cotas: em maio e novembro o banco resgata cotas só para pagar o IR. Informe a quantidade de cotas e o valor da cota da linha do extrato; todo o valor conta como IR."
              : "Lance cada linha do extrato: a quantidade de cotas e o valor da cota do dia. No resgate, informe o IR e o IOF da linha. A cota diária para a Carteira vem do informe que o fundo entrega à CVM. Fundo não paga taxa da B3 e não entra no DARF: o banco já retém o imposto."}
          </p>
        )}

        {!transfer && !form.fund && (
          <label className="inline">
            <input type="checkbox" checked={form.manual} onChange={(e) => set("manual", e.target.checked)} />
            {currency === "USD" || coin
              ? "Informar taxas da corretora"
              : "Informar as taxas à mão (substitui o cálculo automático)"}
          </label>
        )}
        {form.manual && !transfer && !form.fund && (
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
              {form.fund ? (
                <>
                  Valor bruto: <strong>{money(preview.gross, currency)}</strong>
                  {form.side === "VENDA" && (
                    <>
                      {" "}
                      · IR + IOF: <strong>{money(preview.fees, currency)}</strong> · Valor líquido:{" "}
                      <strong>{money(total == null ? null : String(total), currency)}</strong>
                    </>
                  )}
                </>
              ) : (
                <>
                  Valor da operação: <strong>{money(preview.gross, currency)}</strong> · Taxas ({preview.fee_origin}):{" "}
                  <strong>{money(preview.fees, currency)}</strong> · {form.side === "COMPRA" ? "Total pago" : "Total recebido"}:{" "}
                  <strong>{money(total == null ? null : String(total), currency)}</strong>
                </>
              )}
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

      <div className="ops-filters spaced">
        <label>
          Ano
          <select value={String(fYear ?? "")} onChange={(e) => setFYear(e.target.value === "todos" ? "todos" : Number(e.target.value))}>
            {[...years].reverse().map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
            <option value="todos">Todos</option>
          </select>
        </label>
        <label>
          Mês
          <select value={fMonth} onChange={(e) => setFMonth(Number(e.target.value))}>
            <option value={0}>Todos</option>
            {MONTH_NAMES.map((m, i) => (
              <option key={m} value={i + 1}>
                {m}
              </option>
            ))}
          </select>
        </label>
        <label>
          Conta
          <select value={fAccount} onChange={(e) => setFAccount(e.target.value)}>
            <option value="">Todas</option>
            {[...new Set(list.map((p) => p.operation.account))].sort().map((a) => (
              <option key={a} value={a}>
                {a}
              </option>
            ))}
          </select>
        </label>
        <label>
          Ativo
          <input value={fAsset} onChange={(e) => setFAsset(e.target.value)} placeholder="Ex.: ABCD3" />
        </label>
        <span className="muted small">
          {shown.length} lançamento{shown.length === 1 ? "" : "s"}
        </span>
      </div>

      <div className="scroll">
        <table className="ops-table">
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
            {groups.length === 0 && (
              <tr>
                <td colSpan={9} className="muted">
                  {list.length === 0 ? "Nenhum lançamento." : "Nenhum lançamento com esses filtros."}
                </td>
              </tr>
            )}
            {groups.map((g) => {
              if (g.items.length === 1) return row(g.items[0]);
              const isOpen = openNotes.has(g.key) || fAsset.trim() !== "";
              const cur = g.items[0].operation.currency;
              const gross = g.items.reduce((t, p) => t + Number(p.gross), 0);
              const fees = g.items.reduce((t, p) => t + Number(p.fees), 0);
              return (
                <Fragment key={g.key}>
                  <tr className="ops-group clickable" onClick={() => toggleNote(g.key)}>
                    <td>
                      <span className="ret-caret">{isOpen ? "▾" : "▸"}</span>
                      {date(g.items[0].operation.date)}
                    </td>
                    <td>{g.items[0].operation.account}</td>
                    <td colSpan={4}>
                      <strong>{g.items.length} lançamentos neste dia</strong>
                      <span className="muted small"> · {[...new Set(g.items.map((p) => p.operation.asset))].slice(0, 6).join(", ")}{new Set(g.items.map((p) => p.operation.asset)).size > 6 ? "…" : ""}</span>
                    </td>
                    <td className="num">{money(gross.toFixed(2), cur)}</td>
                    <td className="num">{money(fees.toFixed(2), cur)}</td>
                    <td className="row-actions">
                      <button
                        className="link danger"
                        onClick={(e) => {
                          e.stopPropagation();
                          setRemoving(`nota:${g.key}`);
                        }}
                      >
                        Remover todos
                      </button>
                    </td>
                  </tr>
                  {removing === `nota:${g.key}` && (
                    <tr className="detail">
                      <td colSpan={9}>
                        <div className="remove-box">
                          <span>
                            Remover os {g.items.length} lançamentos de {date(g.items[0].operation.date)} em{" "}
                            {g.items[0].operation.account}?
                          </span>
                          <button autoFocus className="danger" onClick={() => confirmRemoveNote(g.items[0].operation)}>
                            Remover todos
                          </button>
                          <button className="secondary" onClick={() => setRemoving(null)}>
                            Cancelar
                          </button>
                        </div>
                      </td>
                    </tr>
                  )}
                  {isOpen && g.items.map(row)}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}
