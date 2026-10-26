import { Fragment, useEffect, useMemo, useState } from "react";
import Intro from "../components/Intro";
import { offerUndo } from "../undo";
import type { EChartsCoreOption } from "echarts/core";
import {
  errorMessage,
  incomesApi,
  market,
  type Account,
  type Income,
  type IncomeKind,
  type IncomeRow,
  type IncomesView,
  type MarketIncomes,
  type SuggestedIncome,
  type PortfolioView,
  type PricedOperation,
} from "../api";
import Chart, { axisStyle, base, usePalette } from "../charts/Chart";
import CashInput from "../components/CashInput";
import { useSort } from "../components/useSort";
import { assetLabel, normalizeTicker, byName } from "../currency";
import { date, money, monthLabel, parseDecimalInput, percent, price, quantity, toCashMask, type Currency } from "../format";
import {
  KIND_LABEL,
  KINDS,
  byAsset,
  byMonth,
  byYear,
  defaultKind,
  foreignTaxDue,
  inYear,
  last12,
  taxGroups,
  totals,
  years as yearsOf,
} from "../incomes";
import { AssetName } from "./Portfolio";
import IncomeForecast from "./IncomeForecast";
import IncomeSuggestions, { suggestionKey } from "./IncomeSuggestions";

type Form = {
  id: string;
  account: string;
  asset: string;
  kind: IncomeKind;
  kindTouched: boolean;
  payDate: string;
  exDate: string;
  gross: string;
  withheld: string;
  note: string;
  reason: string;
};

const today = () => new Date().toLocaleDateString("sv-SE");

const EMPTY: Form = {
  id: "",
  account: "",
  asset: "",
  kind: "DIVIDENDO",
  kindTouched: false,
  payDate: today(),
  exDate: "",
  gross: "",
  withheld: "",
  note: "",
  reason: "",
};

/** Data com: o dia útil anterior à data ex (pula o fim de semana). */
function dayBefore(iso: string): string {
  const d = new Date(`${iso}T12:00:00`);
  do d.setDate(d.getDate() - 1);
  while (d.getDay() === 0 || d.getDay() === 6);
  return d.toLocaleDateString("sv-SE");
}

/** Pagamento estimado: nos EUA, 2 dias úteis depois da data ex; na B3, a própria data ex (o prazo varia muito). */
export function estimatedPayDate(exDate: string, currency: "BRL" | "USD"): string {
  if (currency !== "USD") return exDate;
  const d = new Date(`${exDate}T12:00:00`);
  let left = 2;
  while (left > 0) {
    d.setDate(d.getDate() + 1);
    if (d.getDay() !== 0 && d.getDay() !== 6) left--;
  }
  return d.toLocaleDateString("sv-SE");
}

const SUGGESTED_NOTE = "Pelo histórico do mercado (Yahoo); confira com o extrato";

const KIND_HINT: Record<IncomeKind, string> = {
  DIVIDENDO: "Isento no Brasil. No exterior, normalmente vem com 30% retido nos EUA.",
  JCP: "Juros sobre capital próprio: a empresa já retém 15% de IR.",
  RENDIMENTO: "Rendimento de fundo imobiliário (isento para pessoa física, na maioria dos FIIs).",
  JUROS: "Juros de renda fixa ou saldo remunerado.",
  OUTRO: "Outros valores recebidos pelo ativo (frações de bonificação, restituição de capital…).",
};

function compact(v: number): string {
  const abs = Math.abs(v);
  if (abs >= 1e6) return `R$ ${(v / 1e6).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mi`;
  if (abs >= 1e3) return `R$ ${(v / 1e3).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mil`;
  return `R$ ${v.toLocaleString("pt-BR", { maximumFractionDigits: 0 })}`;
}

const brl = (v: number) => money(v.toFixed(2));

export default function Incomes({ refresh, onChanged }: { refresh: number; onChanged: () => void }) {
  const p = usePalette();
  const [view, setView] = useState<IncomesView | null>(null);
  const [portfolio, setPortfolio] = useState<PortfolioView | null>(null);
  const [ops, setOps] = useState<PricedOperation[]>([]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [form, setForm] = useState<Form>(EMPTY);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [year, setYear] = useState<number | null>(null);
  const [marketData, setMarketData] = useState<MarketIncomes | null>(null);
  const [marketLoading, setMarketLoading] = useState(false);
  const [launched, setLaunched] = useState<Set<string>>(new Set());
  const [filling, setFilling] = useState<string | null>(null);

  function load() {
    incomesApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }

  useEffect(load, [refresh]);
  // Histórico do mercado (Yahoo): ao abrir a tela e depois de remover ou
  // restaurar um provento, para ele voltar (ou sair) da lista dos que faltam lançar.
  function loadMarket(first = false) {
    if (first) setMarketLoading(true);
    incomesApi
      .market()
      .then((m) => {
        setMarketData(m);
        setLaunched(new Set());
      })
      .catch(() => first && setMarketData(null))
      .finally(() => first && setMarketLoading(false));
  }
  useEffect(() => loadMarket(true), []);
  useEffect(() => {
    market.portfolio().then(setPortfolio).catch(() => setPortfolio(null));
    market.listOperations().then(setOps).catch(() => setOps([]));
    market.listAccounts().then(setAccounts).catch(() => setAccounts([]));
  }, [refresh]);

  const rows = view?.items ?? [];
  const allYears = yearsOf(rows);
  // Começa no ano mais recente com proventos.
  useEffect(() => {
    if (year == null && allYears.length) setYear(allYears[0]);
  }, [allYears.join(",")]);

  const names = { ...(portfolio?.names ?? {}), ...(view?.names ?? {}) };
  const kinds = { ...(portfolio?.kinds ?? {}), ...(view?.kinds ?? {}) };
  const account = accounts.find((a) => a.name === form.account.trim());
  const currency: Currency = account?.currency === "USD" ? "USD" : "BRL";

  // Ativos que já passaram pela conta escolhida (ou por qualquer conta).
  const assetOptions = useMemo(() => {
    const acc = form.account.trim();
    const list = ops.filter((o) => !acc || o.operation.account === acc).map((o) => o.operation.asset);
    return [...new Set(list)].sort();
  }, [ops, form.account]);

  function set<K extends keyof Form>(k: K, v: Form[K]) {
    setForm((f) => ({ ...f, [k]: v }));
    setSaved(null);
  }

  function setAsset(raw: string) {
    const asset = normalizeTicker(raw);
    setForm((f) => ({ ...f, asset, kind: f.kindTouched ? f.kind : defaultKind(kinds[asset]) }));
  }

  function edit(i: Income) {
    setForm({
      id: i.id,
      account: i.account,
      asset: i.asset,
      kind: i.kind,
      kindTouched: true,
      payDate: i.pay_date,
      exDate: i.ex_date ?? "",
      gross: toCashMask(i.gross, 2, i.currency).text,
      withheld: Number(i.withheld) ? toCashMask(i.withheld, 2, i.currency).text : "",
      note: i.note ?? "",
      reason: "",
    });
    setSaved(null);
    setError(null);
    window.scrollTo({ top: 0 });
  }

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    const gross = parseDecimalInput(form.gross);
    if (!gross || Number(gross) <= 0) return setError("Informe o valor bruto recebido.");
    if (!account) return setError("Escolha uma conta que já existe (as contas são criadas pelos lançamentos).");
    const income: Income = {
      id: form.id,
      account: form.account.trim(),
      asset: form.asset.trim(),
      kind: form.kind,
      pay_date: form.payDate,
      ex_date: form.exDate || null,
      gross,
      withheld: parseDecimalInput(form.withheld) ?? "0",
      currency,
      note: form.note.trim() || null,
    };
    setBusy(true);
    try {
      if (form.id) await incomesApi.update(income, form.reason);
      else await incomesApi.create(income);
      setSaved(form.id ? "Provento alterado." : "Provento salvo.");
      if (filling && !form.id) setLaunched((l) => new Set(l).add(filling));
      setFilling(null);
      // Na inclusão, o próximo provento já vem com conta, ativo, tipo e datas.
      setForm((f) =>
        f.id
          ? { ...EMPTY, account: f.account, payDate: f.payDate }
          : { ...EMPTY, account: f.account, asset: f.asset, kind: f.kind, kindTouched: f.kindTouched, payDate: f.payDate, exDate: f.exDate },
      );
      setYear(Number(income.pay_date.slice(0, 4)));
      load();
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  /** Preenche o formulário com uma sugestão do histórico do mercado. */
  function fill(sg: SuggestedIncome) {
    setForm({
      ...EMPTY,
      account: sg.account,
      asset: sg.asset,
      kind: sg.kind,
      kindTouched: true,
      payDate: estimatedPayDate(sg.ex_date, sg.currency),
      exDate: dayBefore(sg.ex_date),
      gross: toCashMask(sg.gross, 2, sg.currency).text,
      withheld: Number(sg.withheld) ? toCashMask(sg.withheld, 2, sg.currency).text : "",
      note: SUGGESTED_NOTE,
    });
    setFilling(suggestionKey(sg));
    setSaved(null);
    setError(null);
    window.scrollTo({ top: 0 });
  }

  async function launchAll(list: SuggestedIncome[]) {
    setError(null);
    const done = new Set(launched);
    try {
      for (const sg of list) {
        await incomesApi.create({
          id: "",
          account: sg.account,
          asset: sg.asset,
          kind: sg.kind,
          pay_date: estimatedPayDate(sg.ex_date, sg.currency),
          ex_date: dayBefore(sg.ex_date),
          gross: sg.gross,
          withheld: sg.withheld,
          currency: sg.currency,
          note: SUGGESTED_NOTE,
        });
        done.add(suggestionKey(sg));
      }
      setSaved(`${list.length} provento(s) lançados. Confira as datas de pagamento com o extrato da corretora.`);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setLaunched(done);
      load();
      onChanged();
    }
  }

  async function remove(id: string) {
    try {
      await incomesApi.remove(id);
      if (form.id === id) setForm(EMPTY);
      load();
      loadMarket();
      onChanged();
      offerUndo("Provento removido.", async () => {
        await incomesApi.restore(id);
        load();
        loadMarket();
        onChanged();
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const shown = inYear(rows, year);
  const t = totals(shown);
  const t12 = totals(last12(rows, today()));
  const months =
    year == null
      ? null
      : year === Number(today().slice(0, 4))
        ? Number(today().slice(5, 7))
        : 12;
  const assets = byAsset(shown);
  const groups = year == null ? [] : taxGroups(shown);
  const costBrl = (asset: string) => {
    const pos = portfolio?.positions.find((x) => x.asset === asset && Number(x.quantity) > 0);
    return pos?.cost_brl != null ? Number(pos.cost_brl) : null;
  };
  const net12 = (asset: string) => totals(last12(rows, today()).filter((r) => r.income.asset === asset)).net;
  const assetSort = useSort(
    assets,
    {
      ativo: (a) => (byName(a.asset) && names[a.asset] ? names[a.asset] : a.asset),
      liquido: (a) => a.net,
      pagamentos: (a) => a.count,
      ultimo: (a) => a.last,
      yoc: (a) => {
        const cost = costBrl(a.asset);
        return cost ? net12(a.asset) / cost : null;
      },
    },
    "proventos-ativos",
  );

  const chart = useMemo<EChartsCoreOption | null>(() => {
    if (!rows.length) return null;
    const data = year == null ? byYear(rows) : null;
    const series = year == null ? data!.values : byMonth(rows, year);
    const labels = year == null ? data!.years.map(String) : Array.from({ length: 12 }, (_, i) => monthLabel(i + 1));
    const used = KINDS.filter((k) => series[k].some((v) => v !== 0));
    return {
      ...base(p),
      tooltip: {
        ...(base(p).tooltip as object),
        trigger: "axis",
        valueFormatter: (v: number) => brl(v),
      },
      legend: { ...(base(p).legend as object), data: used.map((k) => KIND_LABEL[k]) },
      xAxis: { type: "category", data: labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: used.map((k) => ({
        name: KIND_LABEL[k],
        type: "bar",
        stack: "p",
        barMaxWidth: 32,
        data: series[k],
        itemStyle: { color: p.series[KINDS.indexOf(k)], borderColor: p.surface, borderWidth: 1 },
      })),
    };
  }, [rows, year, p]);

  // Lista agrupada por mês do pagamento.
  const byMonthList = useMemo(() => {
    const out: { key: string; items: IncomeRow[] }[] = [];
    for (const r of shown) {
      const key = r.income.pay_date.slice(0, 7);
      const last = out[out.length - 1];
      if (last?.key === key) last.items.push(r);
      else out.push({ key, items: [r] });
    }
    return out;
  }, [shown]);

  const net = (() => {
    const g = Number(parseDecimalInput(form.gross) ?? 0);
    const w = Number(parseDecimalInput(form.withheld) ?? 0);
    return g > 0 ? g - w : null;
  })();

  return (
    <section className="incomes">
      <div className="section-head">
        <div>
          <h2>Proventos</h2>
          <Intro
            more={
              <>
                Os proventos ficam somados aqui e não entram na rentabilidade, que mede só a variação de preço. No exterior, os valores em dólar são
                convertidos pela PTAX do dia do pagamento.
              </>
            }
          >
            Dividendos, JCP, rendimentos de FIIs e juros que caíram na conta.
          </Intro>
        </div>
      </div>

      <form className="card op-form" onSubmit={save}>
        <h3>{form.id ? "Alterar provento" : "Novo provento"}</h3>
        <div className="grid">
          <label>
            Corretora (conta)
            <select value={form.account} onChange={(e) => set("account", e.target.value)} required>
              <option value="">Escolha a conta</option>
              {accounts.map((a) => (
                <option key={a.name} value={a.name}>
                  {a.name} {a.currency === "USD" ? "(dólar)" : ""}
                </option>
              ))}
            </select>
          </label>
          <label>
            Ativo (código)
            <input
              list="ativos-proventos"
              value={form.asset}
              onChange={(e) => set("asset", e.target.value.toUpperCase())}
              onBlur={(e) => setAsset(e.target.value)}
              placeholder={currency === "USD" ? "Ex.: VOO" : "Ex.: ABCD3"}
              required
            />
            <datalist id="ativos-proventos">
              {assetOptions.map((a) => (
                <option key={a} value={a}>
                  {names[a] ?? ""}
                </option>
              ))}
            </datalist>
            {names[form.asset] && <span className="muted small">{names[form.asset]}</span>}
          </label>
          <label>
            Tipo
            <select
              value={form.kind}
              onChange={(e) => setForm((f) => ({ ...f, kind: e.target.value as IncomeKind, kindTouched: true }))}
            >
              {KINDS.map((k) => (
                <option key={k} value={k}>
                  {KIND_LABEL[k] === "JCP" ? "JCP (juros sobre capital próprio)" : KIND_LABEL[k]}
                </option>
              ))}
            </select>
          </label>
          <label>
            Data do pagamento
            <input type="date" value={form.payDate} onChange={(e) => set("payDate", e.target.value)} required />
          </label>
          <label>
            Data com (opcional)
            <input type="date" value={form.exDate} onChange={(e) => set("exDate", e.target.value)} />
          </label>
          <label>
            Valor bruto
            <CashInput value={form.gross} decimals={2} currency={currency} onChange={(v) => set("gross", v)} required />
          </label>
          <label>
            Imposto retido (se houver)
            <CashInput value={form.withheld} decimals={2} currency={currency} onChange={(v) => set("withheld", v)} />
          </label>
          <label className="wide">
            Observação (opcional)
            <input value={form.note} onChange={(e) => set("note", e.target.value)} />
          </label>
          {form.id && (
            <label className="wide">
              Motivo da alteração (opcional)
              <input value={form.reason} onChange={(e) => set("reason", e.target.value)} placeholder="Fica registrado na Auditoria" />
            </label>
          )}
        </div>
        <p className="muted small">
          {KIND_HINT[form.kind]}
          {net != null && <> Valor líquido: <strong>{money(net.toFixed(2), currency)}</strong>.</>}
        </p>
        {error && <p className="error">{error}</p>}
        {saved && <p className="ok">{saved}</p>}
        <div className="actions">
          <button type="submit" disabled={busy}>
            {busy ? "Salvando…" : form.id ? "Salvar alteração" : "Salvar provento"}
          </button>
          {form.id && (
            <button type="button" className="secondary" onClick={() => setForm(EMPTY)}>
              Cancelar
            </button>
          )}
        </div>
      </form>

      <IncomeForecast rows={rows} portfolio={portfolio} market={marketData} loading={marketLoading} names={names} />
      {marketData && (
        <IncomeSuggestions
          items={marketData.suggestions.filter((sg) => !launched.has(suggestionKey(sg)))}
          names={names}
          onFill={fill}
          onLaunchAll={() => launchAll(marketData.suggestions.filter((sg) => !launched.has(suggestionKey(sg))))}
        />
      )}

      {rows.length === 0 ? (
        <p className="muted">Nenhum provento lançado ainda.</p>
      ) : (
        <>
          <div className="section-head spaced">
            <h3>Resumo</h3>
            <select value={year ?? ""} onChange={(e) => setYear(e.target.value ? Number(e.target.value) : null)} aria-label="Ano">
              <option value="">Todos os anos</option>
              {allYears.map((y) => (
                <option key={y} value={y}>
                  {y}
                </option>
              ))}
            </select>
          </div>
          <div className="cards">
            <div className="card stat">
              <span className="muted">Líquido recebido</span>
              <strong>{brl(t.net)}</strong>
              {months && <span className="muted small">média de {brl(t.net / months)} por mês</span>}
            </div>
            <div className="card stat">
              <span className="muted">Valor bruto</span>
              <strong>{brl(t.gross)}</strong>
            </div>
            <div className="card stat">
              <span className="muted">Imposto retido</span>
              <strong>{brl(t.withheld)}</strong>
            </div>
            <div className="card stat">
              <span className="muted">Últimos 12 meses</span>
              <strong>{brl(t12.net)}</strong>
              <span className="muted small">líquido</span>
            </div>
          </div>
          {t.missingFx > 0 && (
            <p className="muted small">
              {t.missingFx} provento(s) em dólar ainda sem a PTAX do dia ficaram fora das somas. Atualize as cotações para incluir.
            </p>
          )}

          {chart && (
            <div className="card">
              <h3>{year == null ? "Por ano" : `Por mês em ${year}`}</h3>
              <Chart option={chart} height={280} ariaLabel="Proventos líquidos por período e tipo" />
            </div>
          )}

          <h3 className="spaced">Por ativo</h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  {assetSort.header("ativo", "Ativo")}
                  {assetSort.header("liquido", "Líquido", { numeric: true })}
                  {assetSort.header("pagamentos", "Pagamentos", { numeric: true })}
                  {assetSort.header("ultimo", "Último", { desc: true })}
                  {assetSort.header("yoc", "Rendimento sobre o custo (12 meses)", {
                    numeric: true,
                    title: "Proventos líquidos dos últimos 12 meses sobre o custo atual da posição",
                  })}
                </tr>
              </thead>
              <tbody>
                {assetSort.sorted.map((a) => {
                  const cost = costBrl(a.asset);
                  return (
                    <tr key={a.asset}>
                      <td>
                        <AssetName asset={a.asset} name={names[a.asset]} />
                      </td>
                      <td className="num">{brl(a.net)}</td>
                      <td className="num">{a.count}</td>
                      <td>{date(a.last)}</td>
                      <td className="num">{cost ? percent(String(net12(a.asset) / cost)) : "—"}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {groups.length > 0 && (
            <div className="card spaced">
              <h3>Para a declaração de {year}</h3>
              <ul className="tax-groups">
                {groups.map((g) => (
                  <li key={g.id}>
                    {g.id === "isento" && (
                      <>
                        <strong>Rendimentos isentos e não tributáveis:</strong> {brl(g.net)} de dividendos e rendimentos de FIIs recebidos no
                        Brasil.
                      </>
                    )}
                    {g.id === "exclusivo" && (
                      <>
                        <strong>Rendimentos sujeitos à tributação exclusiva:</strong> {brl(g.net)} líquidos de JCP e juros (bruto{" "}
                        {brl(g.gross)}, com {brl(g.withheld)} de IR já retido).
                      </>
                    )}
                    {g.id === "exterior" && (
                      <>
                        <strong>Exterior (Lei 14.754/2023):</strong> {brl(g.gross)} brutos, com {brl(g.withheld)} retidos lá fora. Entram no
                        ajuste anual com 15%; o imposto pago lá fora pode ser descontado. Estimativa de imposto a pagar:{" "}
                        <strong>{brl(foreignTaxDue(g))}</strong>.
                      </>
                    )}
                  </li>
                ))}
              </ul>
              <p className="muted small">Confira os valores com os informes de rendimentos das corretoras antes de declarar.</p>
            </div>
          )}

          <h3 className="spaced">Pagamentos</h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Pagamento</th>
                  <th>Ativo</th>
                  <th>Tipo</th>
                  <th>Conta</th>
                  <th className="num">Por cota</th>
                  <th className="num">Bruto</th>
                  <th className="num">Retido</th>
                  <th className="num">Líquido</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {byMonthList.map((g) => (
                  <Fragment key={g.key}>
                    <tr className="group-row">
                      <td colSpan={9}>
                        {monthLabel(Number(g.key.slice(5, 7)))} de {g.key.slice(0, 4)} ·{" "}
                        <span className="muted">{brl(totals(g.items).net)} líquidos</span>
                      </td>
                    </tr>
                    {g.items.map((r) => {
                      const i = r.income;
                      return (
                        <tr key={i.id}>
                          <td>
                            {date(i.pay_date)}
                            {i.ex_date && <div className="muted small">data com {date(i.ex_date)}</div>}
                          </td>
                          <td>{byName(i.asset) ? assetLabel(i.asset, names) : i.asset}</td>
                          <td>{KIND_LABEL[i.kind]}</td>
                          <td>{i.account}</td>
                          <td className="num" title={r.quantity ? `${quantity(r.quantity)} cotas na data com` : "Sem posição na data"}>
                            {r.per_share ? price(r.per_share, i.currency) : "—"}
                          </td>
                          <td className="num">{money(i.gross, i.currency)}</td>
                          <td className="num">{Number(i.withheld) ? money(i.withheld, i.currency) : "—"}</td>
                          <td className="num">
                            {money(r.net, i.currency)}
                            {i.currency === "USD" && <div className="muted small">{r.net_brl ? money(r.net_brl) : "sem PTAX"}</div>}
                          </td>
                          <td className="row-actions">
                            <button type="button" className="link" onClick={() => edit(i)}>
                              Alterar
                            </button>
                            <button type="button" className="link subtle" onClick={() => remove(i.id)}>
                              Remover
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                  </Fragment>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}
