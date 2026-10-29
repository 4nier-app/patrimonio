import { useEffect, useState } from "react";
import Intro from "../components/Intro";
import CashInput from "../components/CashInput";
import { offerUndo } from "../undo";
import {
  errorMessage,
  market,
  savingsApi,
  type Account,
  type Savings as SavingsT,
  type SavingsItem,
  type SavingsKind,
  type SavingsMove,
  type SavingsMoveKind,
  type SavingsView,
} from "../api";
import { date, money, parseDecimalInput, toCashMask, tone } from "../format";

const today = () => new Date().toLocaleDateString("sv-SE");

const KINDS: { id: SavingsKind; title: string; text: string; example: string }[] = [
  { id: "COFRINHO", title: "Cofrinho", text: "Caixinha ou conta que rende, com resgate a qualquer hora.", example: "Ex.: Nubank, Mercado Pago, 99Pay" },
  { id: "FGTS", title: "FGTS", text: "Saldo total do FGTS, como aparece no app do FGTS.", example: "Um só, com o total de todas as contas" },
  { id: "CONTA", title: "Conta corrente", text: "Dinheiro parado na conta, sem rendimento.", example: "Ex.: Itaú, Bradesco" },
  { id: "BENEFICIO", title: "Cartão benefício", text: "Saldo de vale-refeição ou alimentação.", example: "Ex.: Swile, Alelo, VR" },
  { id: "DOLAR", title: "Saldo em dólar", text: "Dinheiro parado numa corretora ou conta no exterior, em US$.", example: "Ex.: Nomad (conta investimento, conta global)" },
  { id: "RENDA_FIXA", title: "Renda fixa", text: "CDB, LCI, LCA e outros títulos: o app calcula o saldo pela taxa.", example: "Tesouro Direto com preço automático: Lançamentos › Tesouro Direto" },
];

const PRODUCTS = ["CDB", "LCI", "LCA", "RDB", "LC", "Debênture", "CRI", "CRA", "Tesouro (pelo saldo do extrato)"];

const ISSUERS: Record<SavingsKind, string[]> = {
  COFRINHO: ["Nubank", "Mercado Pago", "99Pay", "PicPay", "Inter", "C6 Bank", "PagBank", "Itaú"],
  FGTS: ["Caixa"],
  CONTA: ["Itaú", "Bradesco", "Banco do Brasil", "Caixa", "Santander", "Nubank", "Inter"],
  BENEFICIO: ["Swile", "Alelo", "VR", "Ticket", "Pluxee", "Caju", "Flash"],
  RENDA_FIXA: ["Itaú", "XP", "BTG", "Rico", "Nubank", "Inter", "Tesouro Direto"],
  DOLAR: ["Nomad", "Avenue", "Wise", "Inter", "C6 Bank"],
};

const NOMAD_ACCOUNTS = ["Conta investimento", "Conta global"];

const NAME_HINT: Record<SavingsKind, string> = {
  COFRINHO: "Ex.: Reserva de emergência",
  FGTS: "",
  CONTA: "Ex.: Conta conjunta (em branco: mostra só o banco)",
  BENEFICIO: "Ex.: Refeição",
  RENDA_FIXA: "Ex.: CDB 2027 (em branco: mostra o produto)",
  DOLAR: "Ex.: Conta investimento ou Conta global",
};

const MOVE_LABEL: Record<SavingsMoveKind, string> = { DEPOSITO: "Depósito", RESGATE: "Resgate", SALDO: "Saldo informado" };

type NewForm = {
  id: string;
  kind: SavingsKind | null;
  account: string;
  name: string;
  rate: string;
  taxFree: boolean;
  /** "AAAA-MM", como no campo de mês. */
  since: string;
  until: string;
  /** Renda fixa. */
  product: string;
  rateDesc: string;
  maturity: string;
  /** Renda fixa nova: a aplicação, lançada junto com o cadastro. */
  applied: string;
  appliedOn: string;
};
const thisMonth = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
};
const EMPTY_NEW = (): NewForm => ({
  id: "",
  kind: null,
  account: "",
  name: "",
  rate: "100",
  taxFree: false,
  since: thisMonth(),
  until: "",
  product: "",
  rateDesc: "",
  maturity: "",
  applied: "",
  appliedOn: "",
});
/** "2024-09-01" → "09/2024". */
const monthLabel = (d: string) => `${d.slice(5, 7)}/${d.slice(0, 4)}`;

type MoveForm = { id: string; savingsId: string; kind: SavingsMoveKind; date: string; amount: string; ir: string; iof: string };

const cash = (v: string) => toCashMask(v, 2, "BRL").text;

/** Cofrinho e renda fixa: entram na carteira, com depósitos e resgates. */
const invests = (k: SavingsKind | null | undefined) => k === "COFRINHO" || k === "RENDA_FIXA";

/** Nome para mensagens: o próprio, ou o tipo e o banco. */
const titleOf = (it: SavingsItem) =>
  it.savings.name && it.savings.name !== it.kind_label
    ? it.savings.name
    : it.savings.fixed?.product
      ? `${it.savings.fixed.product} ${it.savings.account}`
      : `${it.kind_label} ${it.savings.kind === "FGTS" ? "" : it.savings.account}`.trim();

export default function Savings({ onChanged }: { onChanged: () => void }) {
  const [view, setView] = useState<SavingsView | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [form, setForm] = useState<NewForm | null>(null);
  // Como o app entendeu a taxa digitada na renda fixa.
  const [rateRead, setRateRead] = useState<string | null>(null);
  useEffect(() => {
    const text = form?.rateDesc.trim() ?? "";
    if (!text) return setRateRead(null);
    savingsApi
      .describeRate(text)
      .then(setRateRead)
      .catch(() => setRateRead(null));
  }, [form?.rateDesc]);
  const [move, setMove] = useState<MoveForm | null>(null);
  const [lastDate, setLastDate] = useState(today());
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function load() {
    savingsApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }

  useEffect(load, []);
  useEffect(() => {
    market.listAccounts().then(setAccounts).catch(() => setAccounts([]));
  }, []);

  function changed(message: string) {
    setSaved(message);
    load();
    onChanged();
  }

  // O app guarda os dados de uma pessoa: um FGTS só, e uma conta (ou cartão
  // benefício) por banco. Cofrinhos podem ser vários no mesmo banco; do mesmo
  // banco, só um fica sem nome.
  const all = view?.items ?? [];
  const unnamed = all.filter((it) => !it.savings.name || it.savings.name === it.kind_label);
  const bankOf = (kind: SavingsKind, account: string) => (kind === "FGTS" ? "" : account.trim().toLowerCase());
  const sameSlot = (it: SavingsItem) =>
    !!form?.kind &&
    it.savings.id !== form.id &&
    it.savings.kind === form.kind &&
    bankOf(it.savings.kind, it.savings.account) === bankOf(form.kind!, form.account);
  // Cofrinhos e saldos em dólar podem ser vários na mesma empresa (na Nomad:
  // conta investimento e conta global).
  const isCofrinho = form?.kind === "COFRINHO" || form?.kind === "DOLAR";
  const fgtsTaken = all.some((it) => it.savings.kind === "FGTS");
  const needsName = isCofrinho && unnamed.some(sameSlot);
  // Na Nomad o saldo em dólar é a conta investimento ou a conta global, uma de cada.
  const isNomad = form?.kind === "DOLAR" && form.account.trim().toLowerCase() === "nomad";
  const nameOf = (n: string) => n.trim().toLowerCase();
  const nameTaken = (n: string) => all.some((it) => sameSlot(it) && !!nameOf(n) && nameOf(it.savings.name) === nameOf(n));
  const sameName = form?.kind === "DOLAR" && nameTaken(form.name);
  // Um cadastro antigo repetido continua podendo ser alterado.
  const original = all.find((it) => it.savings.id === form?.id)?.savings;
  const kept = !!original && original.kind === form?.kind && bankOf(original.kind, original.account) === bankOf(form.kind, form.account);
  // Renda fixa: quantos títulos quiser, em qualquer banco.
  const isFixed = form?.kind === "RENDA_FIXA";
  const duplicate = !!form?.kind && !isCofrinho && !isFixed && !kept && all.some(sameSlot);
  // Conta, FGTS e benefício não têm nome (só os cadastros antigos que já tinham).
  const hadName = !!form?.id && !unnamed.some((it) => it.savings.id === form.id);
  const showName = isCofrinho || isFixed || hadName;

  async function saveSavings(e: React.FormEvent) {
    e.preventDefault();
    if (!form?.kind) return;
    setError(null);
    const item: SavingsT = {
      id: form.id,
      kind: form.kind,
      // FGTS não tem banco.
      account: form.kind === "FGTS" ? "" : form.account.trim(),
      name: showName ? form.name.trim() : "",
      rate_pct: parseDecimalInput(form.rate) ?? "100",
      tax_free: form.taxFree,
      note: null,
      since: form.since ? `${form.since}-01` : null,
      until: form.until ? `${form.until}-01` : null,
      fixed:
        form.kind === "RENDA_FIXA"
          ? { product: form.product.trim(), rate: form.rateDesc.trim() || null, maturity: form.maturity || null }
          : null,
    };
    setBusy(true);
    try {
      if (form.id) {
        await savingsApi.update(item);
        changed("Alteração salva.");
      } else {
        const created = await savingsApi.create(item);
        const applied = form.kind === "RENDA_FIXA" ? parseDecimalInput(form.applied) : null;
        if (applied != null && Number(applied) > 0 && form.appliedOn) {
          await savingsApi.createMove({
            id: "",
            savings_id: created.id,
            date: form.appliedOn,
            kind: "DEPOSITO",
            amount: applied,
            ir: null,
            iof: null,
            note: null,
          });
        }
        changed(
          form.kind === "RENDA_FIXA" && rateRead && applied != null
            ? `${item.fixed?.product} cadastrado. O app calcula o saldo todo dia pela taxa (${rateRead}).`
            : `${item.name || item.fixed?.product || "Saldo"} cadastrado. Informe o saldo de cada mês em Evolução › Patrimônio, clicando na célula.`,
        );
      }
      setForm(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function saveMove(e: React.FormEvent) {
    e.preventDefault();
    if (!move) return;
    setError(null);
    const amount = parseDecimalInput(move.amount);
    if (amount == null || (move.kind !== "SALDO" && Number(amount) <= 0)) return setError("Informe o valor.");
    const item: SavingsMove = {
      id: move.id,
      savings_id: move.savingsId,
      date: move.date,
      kind: move.kind,
      amount,
      ir: move.kind === "RESGATE" ? parseDecimalInput(move.ir) : null,
      iof: move.kind === "RESGATE" ? parseDecimalInput(move.iof) : null,
      note: null,
    };
    setBusy(true);
    try {
      if (move.id) await savingsApi.updateMove(item);
      else await savingsApi.createMove(item);
      setLastDate(move.date);
      setMove(null);
      setOpen((o) => ({ ...o, [move.savingsId]: true }));
      changed(move.id ? "Movimento alterado." : `${MOVE_LABEL[move.kind]} salvo.`);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function removeSavings(it: SavingsItem) {
    try {
      await savingsApi.remove(it.savings.id);
      changed("");
      setSaved(null);
      offerUndo(`${titleOf(it)} removido.`, async () => {
        await savingsApi.restore(it.savings.id);
        load();
        onChanged();
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function removeMove(m: SavingsMove) {
    try {
      await savingsApi.removeMove(m.id);
      changed("");
      setSaved(null);
      offerUndo("Movimento removido.", async () => {
        await savingsApi.restoreMove(m.id);
        load();
        onChanged();
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function startMove(it: SavingsItem, kind: SavingsMoveKind) {
    setMove({ id: "", savingsId: it.savings.id, kind, date: kind === "SALDO" ? today() : lastDate, amount: "", ir: "", iof: "" });
    setSaved(null);
    setError(null);
  }

  function editMove(m: SavingsMove) {
    setMove({
      id: m.id,
      savingsId: m.savings_id,
      kind: m.kind,
      date: m.date,
      amount: cash(m.amount),
      ir: m.ir != null ? cash(m.ir) : "",
      iof: m.iof != null ? cash(m.iof) : "",
    });
    setSaved(null);
    setError(null);
  }

  function editSavings(s: SavingsT) {
    // Cadastros antigos guardavam o tipo como nome; aqui ele aparece em branco.
    const own = s.name === KINDS.find((k) => k.id === s.kind)?.title ? "" : s.name;
    setForm({
      id: s.id,
      kind: s.kind,
      account: s.account,
      name: own,
      rate: s.rate_pct,
      taxFree: s.tax_free,
      since: s.since?.slice(0, 7) ?? thisMonth(),
      until: s.until?.slice(0, 7) ?? "",
      product: s.fixed?.product ?? "",
      rateDesc: s.fixed?.rate ?? "",
      maturity: s.fixed?.maturity ?? "",
      applied: "",
      appliedOn: "",
    });
    setSaved(null);
    window.scrollTo({ top: 0 });
  }

  const items = view?.items ?? [];
  // No saldo em dólar, primeiro as corretoras em dólar já usadas nos lançamentos.
  const accountNames = [
    ...new Set([
      ...(form?.kind === "DOLAR" ? accounts.filter((a) => a.currency === "USD").map((a) => a.name) : []),
      ...(form?.kind ? ISSUERS[form.kind] : []),
      ...(form?.kind === "DOLAR" ? [] : accounts.map((a) => a.name)),
    ]),
  ];
  const cur = (it: SavingsItem): "BRL" | "USD" => (it.savings.kind === "DOLAR" ? "USD" : "BRL");

  function moveForm(it: SavingsItem) {
    if (!move || move.savingsId !== it.savings.id) return null;
    return (
      <form className="savings-move op-form" onSubmit={saveMove}>
        <strong>{move.id ? `Alterar ${MOVE_LABEL[move.kind].toLowerCase()}` : MOVE_LABEL[move.kind]}</strong>
        <div className="grid">
          {move.id && invests(it.savings.kind) && (
            <label>
              Tipo
              <select value={move.kind} onChange={(e) => setMove({ ...move, kind: e.target.value as SavingsMoveKind })}>
                {(Object.keys(MOVE_LABEL) as SavingsMoveKind[]).map((k) => (
                  <option key={k} value={k}>
                    {MOVE_LABEL[k]}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label>
            Data
            <input type="date" value={move.date} max={today()} onChange={(e) => setMove({ ...move, date: e.target.value })} required />
          </label>
          <label>
            {move.kind === "SALDO"
              ? cur(it) === "USD"
                ? "Saldo em dólar que o app mostra"
                : "Saldo que o app do banco mostra"
              : move.kind === "DEPOSITO"
                ? "Valor depositado"
                : "Valor que caiu na conta"}
            <CashInput value={move.amount} decimals={2} currency={cur(it)} onChange={(v) => setMove({ ...move, amount: v })} required />
          </label>
          {move.kind === "RESGATE" && invests(it.savings.kind) && (
            <>
              <label>
                IR retido (opcional)
                <CashInput value={move.ir} decimals={2} currency="BRL" onChange={(v) => setMove({ ...move, ir: v })} />
              </label>
              <label>
                IOF retido (opcional)
                <CashInput value={move.iof} decimals={2} currency="BRL" onChange={(v) => setMove({ ...move, iof: v })} />
              </label>
            </>
          )}
        </div>
        {move.kind === "RESGATE" && invests(it.savings.kind) && (
          <p className="muted small">Se o banco não mostrar o IR e o IOF, deixe em branco.</p>
        )}
        <div className="actions">
          <button type="submit" disabled={busy}>
            {busy ? "Salvando…" : "Salvar"}
          </button>
          <button type="button" className="secondary" onClick={() => setMove(null)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  }

  function detail(it: SavingsItem, m: SavingsMove): string {
    if (m.kind === "SALDO") {
      const c = it.checks.find((x) => x.move_id === m.id);
      if (!c) return "saldo inicial";
      const d = Number(c.difference);
      if (Math.abs(d) < 0.005) return "igual ao esperado";
      return `esperado ${money(c.expected)}, diferença ${d > 0 ? "+" : ""}${money(c.difference)}`;
    }
    if (m.kind === "RESGATE") {
      const r = it.redemptions.find((x) => x.move_id === m.id);
      if (!r || (Number(r.ir) === 0 && Number(r.iof) === 0)) return "";
      return `bruto ${money(r.gross)} · IR ${money(r.ir)} · IOF ${money(r.iof)}`;
    }
    return "";
  }

  return (
    <section className="savings">
      <div className="section-head">
        <div>
          <h2>Contas e reservas</h2>
          <Intro
            more={
              <>
                O que vale é o saldo que o banco mostra: informe o saldo de cada mês clicando no mês, em Evolução › Patrimônio. O app não estima nada
                entre um saldo e outro. No cofrinho, lance também os depósitos e resgates, para o rendimento não contar dinheiro novo; se o saldo informado
                ficar muito longe do esperado, o app avisa, porque provavelmente faltou lançar um deles. Na renda fixa (CDB, LCI, LCA) com a taxa
                informada (por exemplo 102% do CDI, 12% ao ano ou IPCA + 6%), o app calcula o saldo todo dia pelo CDI e pelo IPCA do Banco Central,
                mostra quanto sobraria já sem o IR e, no vencimento, tira o valor da carteira sozinho. Se você informar o saldo do extrato, ele vale
                mais que o calculado. Cofrinhos e renda fixa
                entram na carteira e na rentabilidade; FGTS, conta corrente, cartões benefício e saldos em dólar entram só no patrimônio. O saldo em dólar é informado em US$ e convertido pelo câmbio da corretora (valor de resgate ou PTAX). O saldo de 31/12 de cada um aparece em Impostos, para Bens e Direitos.
              </>
            }
          >
            Cofrinhos, renda fixa, FGTS, conta corrente, cartões benefício e saldos em dólar, pelo saldo que você vê no app do banco.
          </Intro>
        </div>
        {!form && (
          <button onClick={() => setForm(EMPTY_NEW())} type="button">
            + Novo saldo
          </button>
        )}
      </div>

      {form && (
        <form className="card op-form" onSubmit={saveSavings}>
          <h3>{form.id ? "Alterar saldo" : "1. O que você quer acompanhar?"}</h3>
          {!form.id && (
            <div className="event-kinds">
              {KINDS.map((k) => {
                const taken = k.id === "FGTS" && fgtsTaken;
                return (
                  <button
                    key={k.id}
                    type="button"
                    className={`event-kind${form.kind === k.id ? " active" : ""}`}
                    disabled={taken}
                    onClick={() => setForm({ ...form, kind: k.id })}
                  >
                    <strong>{k.title}</strong>
                    <span>{k.text}</span>
                    <span className="muted small">{taken ? "Já cadastrado" : k.example}</span>
                  </button>
                );
              })}
            </div>
          )}
          {form.kind && (
            <>
              {!form.id && <h3 className="spaced">2. Dados</h3>}
              <div className="grid">
                {form.kind !== "FGTS" && (
                <label>
                  {form.kind === "BENEFICIO"
                    ? "Empresa do cartão"
                    : form.kind === "RENDA_FIXA" || form.kind === "DOLAR"
                      ? "Banco ou corretora"
                      : "Banco"}
                  <input list="saldos-bancos" value={form.account} onChange={(e) => setForm({ ...form, account: e.target.value })} required />
                  <datalist id="saldos-bancos">
                    {accountNames.map((a) => (
                      <option key={a} value={a} />
                    ))}
                  </datalist>
                </label>
                )}
                {isFixed && (
                  <>
                    <label>
                      Produto
                      <input list="saldos-produtos" value={form.product} onChange={(e) => setForm({ ...form, product: e.target.value })} required />
                      <datalist id="saldos-produtos">
                        {PRODUCTS.map((p) => (
                          <option key={p} value={p} />
                        ))}
                      </datalist>
                    </label>
                    <label>
                      Taxa
                      <input value={form.rateDesc} placeholder="Ex.: 102% do CDI, 12% ao ano, IPCA + 6%" onChange={(e) => setForm({ ...form, rateDesc: e.target.value })} />
                      <span className="muted small">
                        {!form.rateDesc.trim()
                          ? "Com a taxa, o app calcula o saldo todo dia pelo CDI e pelo IPCA do Banco Central."
                          : rateRead
                            ? `Entendi: ${rateRead}. O saldo é calculado todo dia.`
                            : "Não entendi essa taxa: o saldo fica só pelo que você informar do extrato."}
                      </span>
                    </label>
                    <label>
                      Vencimento
                      <input type="date" value={form.maturity} onChange={(e) => setForm({ ...form, maturity: e.target.value })} />
                      <span className="muted small">No vencimento, o valor sai sozinho, já sem o IR.</span>
                    </label>
                    {!form.id && (
                      <>
                        <label>
                          Valor aplicado
                          <CashInput value={form.applied} decimals={2} currency="BRL" onChange={(v) => setForm({ ...form, applied: v })} />
                        </label>
                        <label>
                          Data da aplicação
                          <input
                            type="date"
                            value={form.appliedOn}
                            onChange={(e) => setForm({ ...form, appliedOn: e.target.value, since: e.target.value.slice(0, 7) || form.since })}
                          />
                        </label>
                      </>
                    )}
                  </>
                )}
                {showName && isNomad && (
                  <label>
                    Qual conta
                    <select value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} required>
                      <option value="">Escolha</option>
                      {NOMAD_ACCOUNTS.map((n) => (
                        <option key={n} value={n} disabled={nameTaken(n)}>
                          {n}
                          {nameTaken(n) ? " (já cadastrada)" : ""}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
                {showName && !isNomad && (
                  <label>
                    {needsName ? `Nome (para diferenciar do outro ${form.kind === "DOLAR" ? "saldo" : "cofrinho"})` : "Nome (opcional)"}
                    <input
                      value={form.name}
                      placeholder={NAME_HINT[form.kind]}
                      onChange={(e) => setForm({ ...form, name: e.target.value })}
                      required={needsName}
                    />
                  </label>
                )}
                <label>
                  Desde (mês e ano)
                  <input
                    type="month"
                    value={form.since}
                    max={form.until || undefined}
                    onChange={(e) => setForm({ ...form, since: e.target.value })}
                    required
                  />
                </label>
                <label>
                  Até (só se encerrada)
                  <input
                    type="month"
                    value={form.until}
                    min={form.since || undefined}
                    onChange={(e) => setForm({ ...form, until: e.target.value })}
                  />
                </label>
                {form.kind === "DOLAR" && (
                  <p className="muted small">
                    Informe o saldo em US$, como aparece no app. O app converte para reais pelo câmbio da corretora: com o custo para trazer ao Brasil
                    informado em Lançamentos (na Nomad, 1,38%), pelo valor de resgate; sem ele, pela PTAX. O saldo entra no patrimônio, não na
                    rentabilidade.
                  </p>
                )}
                <p className="muted small">
                  Entre esses meses, a linha aparece em Evolução mesmo vazia, para você informar os saldos. Depois do
                  "até", o saldo some e não conta mais no patrimônio.
                </p>
              </div>
            </>
          )}
          {sameName && <p className="error">Já existe um saldo em dólar {form.account.trim()} com esse nome.</p>}
          {duplicate && (
            <p className="error">
              {form.kind === "FGTS"
                ? "Já existe um FGTS cadastrado."
                : `Já existe um cadastro de ${KINDS.find((k) => k.id === form.kind)?.title.toLowerCase()} ${form.account.trim()}.`}{" "}
              O app guarda os dados de uma pessoa; para outra pessoa, crie outro perfil na tela de entrada.
            </p>
          )}
          {error && <p className="error">{error}</p>}
          <div className="actions">
            {form.kind && (
              <button type="submit" disabled={busy || duplicate || sameName}>
                {busy ? "Salvando…" : form.id ? "Salvar alteração" : "Cadastrar"}
              </button>
            )}
            <button type="button" className="secondary" onClick={() => setForm(null)}>
              Cancelar
            </button>
          </div>
        </form>
      )}

      {!form && error && <p className="error">{error}</p>}
      {saved && <p className="ok">{saved}</p>}

      {view && items.length > 0 && (
        <div className="savings-totals">
          <div>
            <span className="muted small">Cofrinhos e renda fixa (investimento)</span>
            <strong>{money(view.total_investments)}</strong>
          </div>
          <div>
            <span className="muted small">Outros saldos (FGTS, conta, benefício, dólar)</span>
            <strong>{money(view.total_others)}</strong>
          </div>
        </div>
      )}

      {!view ? (
        <p className="muted">Carregando…</p>
      ) : items.length === 0 ? (
        !form && <p className="muted">Nenhum saldo cadastrado ainda. Clique em "Novo saldo" para começar.</p>
      ) : (
        <div className="savings-list">
          {items.map((it) => {
            const s = it.savings;
            const lastCheck = it.checks[it.checks.length - 1];
            const isCofrinho = invests(s.kind);
            return (
              <div className="card savings-card" key={s.id}>
                <div className="savings-head">
                  <div>
                    <span className="tag">{it.kind_label}</span>
                    <h3>
                      {s.name && s.name !== it.kind_label ? (
                        <>
                          {s.name}
                          {s.kind !== "FGTS" && s.account && <span className="muted"> · {s.account}</span>}
                        </>
                      ) : s.kind === "FGTS" ? (
                        it.kind_label
                      ) : s.fixed ? (
                        <>
                          {s.fixed.product}
                          <span className="muted"> · {s.account}</span>
                        </>
                      ) : (
                        s.account || it.kind_label
                      )}
                    </h3>
                    {s.fixed && (s.fixed.rate || s.fixed.maturity) && (
                      <p className="muted small">
                        {[s.fixed.rate, s.fixed.maturity && `vence em ${date(s.fixed.maturity)}`].filter(Boolean).join(" · ")}
                      </p>
                    )}
                    {it.usd_note && <p className="muted small" style={{ whiteSpace: "pre-line" }}>{it.usd_note}</p>}
                    {it.automatic && (
                      <p className="small">
                        Saldo calculado todo dia pela taxa ({it.automatic}).
                        {Number(it.balance_net) !== Number(it.balance) && ` Resgatando hoje, já sem IR e IOF: ${money(it.balance_net)}.`}
                      </p>
                    )}
                    <p className="muted small">
                      {s.since && `Desde ${monthLabel(s.since)}`}
                      {s.until && ` · encerrada em ${monthLabel(s.until)}`}
                      {s.since && " · "}
                      {it.last_informed ? `Último saldo informado em ${date(it.last_informed)}` : "Nenhum saldo informado ainda"}
                    </p>
                  </div>
                  <div className="savings-balance">
                    <span className="muted small">{isCofrinho ? "Saldo" : "Último saldo informado"}</span>
                    {it.usd != null && <strong>{money(it.usd, "USD")}</strong>}
                    {it.usd != null ? (
                      <span className="small" title={it.usd_note ?? undefined}>
                        {money(it.balance)} em reais
                      </span>
                    ) : (
                      <strong>{money(it.balance)}</strong>
                    )}
                    {isCofrinho && Number(it.earned) !== 0 && (
                      <span className={`small ${tone(it.earned)}`}>
                        rendimento {money(it.earned)}
                      </span>
                    )}
                  </div>
                </div>

                {lastCheck?.suspicious && (
                  <p className="banner warning small">
                    O saldo de {date(lastCheck.date)} ficou {money(String(Math.abs(Number(lastCheck.difference))))}{" "}
                    {Number(lastCheck.difference) > 0 ? "acima" : "abaixo"} do esperado. Faltou lançar algum{" "}
                    {Number(lastCheck.difference) > 0 ? "depósito" : "resgate"}? Se não, pode ignorar: o app já usa o saldo informado.
                  </p>
                )}

                {!move || move.savingsId !== s.id ? (
                  <div className="actions">
                    <button type="button" onClick={() => startMove(it, "SALDO")}>
                      Atualizar saldo
                    </button>
                    {isCofrinho && (
                      <>
                        <button type="button" className="secondary" onClick={() => startMove(it, "DEPOSITO")}>
                          Depósito
                        </button>
                        <button type="button" className="secondary" onClick={() => startMove(it, "RESGATE")}>
                          Resgate
                        </button>
                      </>
                    )}
                    <span className="spacer" />
                    {isCofrinho && it.moves.length > 0 && (
                      <button type="button" className="link" onClick={() => setOpen((o) => ({ ...o, [s.id]: !o[s.id] }))}>
                        {open[s.id] ? "Esconder movimentos" : `Ver movimentos (${it.moves.length})`}
                      </button>
                    )}
                    <button type="button" className="link" onClick={() => editSavings(s)}>
                      Alterar
                    </button>
                    <button type="button" className="link subtle" onClick={() => removeSavings(it)}>
                      Remover
                    </button>
                  </div>
                ) : (
                  moveForm(it)
                )}

                {isCofrinho && open[s.id] && it.moves.length > 0 && (
                  <div className="scroll">
                    <table>
                      <thead>
                        <tr>
                          <th>Data</th>
                          <th>Movimento</th>
                          <th className="num">Valor</th>
                          <th>Detalhe</th>
                          <th />
                        </tr>
                      </thead>
                      <tbody>
                        {it.moves.map((m) => (
                          <tr key={m.id}>
                            <td>{date(m.date)}</td>
                            <td>{MOVE_LABEL[m.kind]}</td>
                            <td className="num">{money(m.amount)}</td>
                            <td className="muted small">{detail(it, m)}</td>
                            <td className="row-actions">
                              <button type="button" className="link" onClick={() => editMove(m)}>
                                Alterar
                              </button>
                              <button type="button" className="link subtle" onClick={() => removeMove(m)}>
                                Remover
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
