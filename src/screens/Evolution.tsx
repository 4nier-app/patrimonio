import { createContext, Fragment, useContext, useEffect, useState } from "react";
import YearPager from "../components/YearPager";
import { errorMessage, evolutionApi, savingsApi, type Dec, type EvolutionColumn, type EvolutionGain, type EvolutionView } from "../api";
import CashInput from "../components/CashInput";
import Intro from "../components/Intro";
import { cashZero, date, money, monthLabel, parseDecimalInput, percent, toCashMask, tone } from "../format";

// Na tabela, valores sem "R$" (o cabeçalho já diz) para caber o ano inteiro.
const numFmt = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const short = (v: Dec) => numFmt.format(Number(v));

const COLLAPSED_KEY = "4nier.evolucao.fechados";

function loadCollapsed(): string[] {
  try {
    return JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]") as string[];
  } catch {
    return [];
  }
}

function saveCollapsed(ids: string[]) {
  try {
    localStorage.setItem(COLLAPSED_KEY, JSON.stringify(ids));
  } catch {
    // Sem armazenamento local: os grupos só não ficam lembrados.
  }
}

export function columnLabel(c: EvolutionColumn): { top: string; sub?: string } {
  const [y, m, d] = c.date.split("-");
  switch (c.kind) {
    case "BASE":
      return { top: `31/12/${y.slice(2)}`, sub: "início" };
    case "MONTH":
      return { top: monthLabel(Number(m)), sub: `${d}/${m}` };
    case "YEAR":
      return { top: y, sub: "31/12" };
    case "YESTERDAY":
      return { top: "ontem", sub: `${d}/${m}` };
    case "TODAY":
      return { top: "hoje", sub: `${d}/${m}` };
  }
}

/** Variação simples em relação à coluna anterior: valor ÷ anterior − 1. */
/** Colunas que cada coluna compara: [de, até]. */
type Pair = [number, number] | null;

/**
 * Em geral, cada coluna compara com a anterior. No ano corrente, "Hoje"
 * compara com o fim do mês anterior (como o Painel) e "Ontem" mostra a
 * evolução de ontem para hoje.
 */
function comparisons(cols: EvolutionColumn[]): Pair[] {
  return cols.map((c, i) => {
    if (c.kind === "YESTERDAY" && cols[i + 1]?.kind === "TODAY") return [i, i + 1];
    if (c.kind === "TODAY" && cols[i - 1]?.kind === "YESTERDAY") {
      let j = i - 2;
      while (j >= 0 && cols[j].kind !== "MONTH" && cols[j].kind !== "BASE") j--;
      return j >= 0 ? [j, i] : null;
    }
    return i > 0 ? [i - 1, i] : null;
  });
}

/** Ordem na tela: "Ontem" depois de "Hoje", como na planilha. */
function displayOrder(cols: EvolutionColumn[]): number[] {
  const order = cols.map((_, i) => i);
  const n = cols.length;
  if (n >= 2 && cols[n - 2].kind === "YESTERDAY" && cols[n - 1].kind === "TODAY") [order[n - 2], order[n - 1]] = [n - 1, n - 2];
  return order;
}

const consecutive = (n: number): Pair[] => Array.from({ length: n }, (_, i) => (i > 0 ? [i - 1, i] : null));

const Pairs = createContext<Pair[]>([]);
/** Índice da coluna "Hoje", que fica destacada. */
const Today = createContext(-1);

function change(values: (Dec | null)[], pair: Pair): string | null {
  if (!pair) return null;
  const prev = values[pair[0]];
  const cur = values[pair[1]];
  if (cur == null || prev == null || Number(prev) === 0) return null;
  return String(Number(cur) / Number(prev) - 1);
}

function Cell({
  values,
  i,
  missing,
  strong,
  noRate,
  onEdit,
  note,
}: {
  values: (Dec | null)[];
  i: number;
  missing?: boolean;
  /** Conta do valor de resgate (corretora em dólar), mostrada no ⓘ. */
  note?: string | null;
  strong?: boolean;
  /** Sem a % de variação (total com bens). */
  noRate?: boolean;
  /** Saldo: clicar abre o campo para informar o valor nessa data. */
  onEdit?: () => void;
}) {
  const pairs = useContext(Pairs);
  const hl = useContext(Today) === i ? " today" : "";
  const v = values[i];
  const edit = onEdit ? { className: `num evo-cell editable${hl}`, onClick: onEdit, title: `${v == null ? "" : `${money(v)} · `}clique para informar o saldo` } : null;
  if (v == null)
    return edit ? (
      <td {...edit}>
        <span className="evo-add">+</span>
      </td>
    ) : (
      <td className={`num evo-cell${hl}`}>{missing && <span className="ret-missing">sem cotação</span>}</td>
    );
  const pct = noRate ? null : change(values, pairs[i] ?? (i > 0 ? [i - 1, i] : null));
  return (
    <td className={edit?.className ?? `num evo-cell${hl}`} title={edit?.title ?? (note ? `${money(v)} · valor de resgate\n${note}` : money(v))} onClick={onEdit}>
      <span className={strong ? "evo-val strong" : "evo-val"}>
        {short(v)}
        {note && <span className="info-mark"> ⓘ</span>}
      </span>
      {pct != null && <span className={`evo-pct ${tone(pct)}`}>{percent(pct)}</span>}
    </td>
  );
}

/** Resultado no período, descontando compras e vendas. */
function Gain({ gain, strong, plain }: { gain: EvolutionGain | null; strong?: boolean; plain?: boolean }) {
  const rate = gain && Number(gain.base) > 0 ? String(Number(gain.value) / Number(gain.base)) : null;
  return (
    <td className={`num evo-cell ${plain ? "" : "evo-gain"}`} title={gain ? money(gain.value) : undefined}>
      {gain && (
        <>
          <span className={`evo-val ${strong ? "strong" : ""} ${tone(gain.value)}`}>{short(gain.value)}</span>
          {rate != null && <span className={`evo-pct ${tone(rate)}`}>{percent(rate)}</span>}
        </>
      )}
    </td>
  );
}

/** Linha só de valores em R$ (variação, rendimento), sem %. */
function FlowRow({
  label,
  hint,
  values,
  total,
  highlight,
}: {
  label: string;
  hint: string;
  values: (string | null)[];
  /** Posição na tela da coluna "Hoje". */
  highlight: number;
  /** Valores coluna a coluna para o total do período (sem repetir dias). */
  total: (string | null)[];
}) {
  const known = total.filter((v): v is string => v != null);
  const sum = known.length ? known.reduce((s, v) => s + Number(v), 0).toFixed(2) : null;
  return (
    <tr className="evo-flow">
      <th scope="row" title={hint}>
        {label}
      </th>
      {values.map((v, i) => (
        <td key={i} className={`num evo-cell${i === highlight ? " today" : ""}`} title={v == null ? undefined : money(v)}>
          {v != null && <span className={`evo-val ${tone(v)}`}>{short(v)}</span>}
        </td>
      ))}
      <td className="num evo-cell evo-gain" title={sum == null ? undefined : money(sum)}>
        {sum != null && <span className={`evo-val ${tone(sum)}`}>{short(sum)}</span>}
      </td>
      <td className="num evo-cell" />
    </tr>
  );
}

/** Diferença para a coluna anterior. */
/** Total geral sem os bens (imóveis e veículos), que entram pelo custo. */
export function withoutOwned(view: EvolutionView): { values: (Dec | null)[]; gain: EvolutionGain | null } | null {
  const owned = view.groups.filter((g) => g.id === "bens");
  if (!owned.length) return null;
  const values = view.total.map((t, i) => {
    if (t == null) return null;
    const b = owned.reduce((s, g) => s + Number(g.total[i] ?? 0), 0);
    return (Number(t) - b).toFixed(2);
  });
  const first = values[0];
  const last = values[values.length - 1];
  const gain =
    first == null && last == null
      ? null
      : { value: (Number(last ?? 0) - Number(first ?? 0)).toFixed(2), base: first ?? "0" };
  return { values, gain };
}

const diffs = (values: (Dec | null)[], pairs: Pair[]): (string | null)[] =>
  pairs.map((pair) => {
    if (!pair) return null;
    const prev = values[pair[0]];
    const v = values[pair[1]];
    if (v == null && prev == null) return null;
    return (Number(v ?? 0) - Number(prev ?? 0)).toFixed(2);
  });

/** Rendimento: variação dos investimentos menos o dinheiro novo no intervalo. */
const incomes = (view: EvolutionView, pairs: Pair[]): (string | null)[] =>
  diffs(view.investments, pairs).map((d, i) => {
    const pair = pairs[i];
    if (d == null || !pair) return null;
    let n = 0;
    for (let k = pair[0] + 1; k <= pair[1]; k++) {
      const m = view.new_money[k];
      if (m == null) return null;
      n += Number(m);
    }
    return (Number(d) - n).toFixed(2);
  });

export default function Evolution() {
  const [year, setYear] = useState<number | null | undefined>(undefined);
  const [view, setView] = useState<EvolutionView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<string[]>(loadCollapsed);
  const [edit, setEdit] = useState<Editing | null>(null);
  const [reload, setReload] = useState(0);

  useEffect(() => {
    // Primeira carga: ano atual. Depois, o ano escolhido (null = todos).
    const wanted = year === undefined ? new Date().getFullYear() : year;
    setError(null);
    evolutionApi
      .load(wanted)
      .then((v) => {
        setView(v);
        if (year === undefined) setYear(v.year ?? v.years[0] ?? null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [year, reload]);

  function toggle(key: string) {
    const next = collapsed.includes(key) ? collapsed.filter((k) => k !== key) : [...collapsed, key];
    setCollapsed(next);
    saveCollapsed(next);
  }

  if (error) return <p className="error">{error}</p>;
  if (!view) return <p className="muted">Calculando…</p>;

  const head = (
    <div className="section-head">
      <div>
        <h2>Evolução do patrimônio</h2>
        <Intro
          more={
            <>
              Cada coluna é o patrimônio no fim do mês, em reais; o que está em dólar é convertido pela PTAX do dia. Nas corretoras em dólar com o custo para trazer ao Brasil informado, vale o valor de resgate: dólar comercial do dia menos a taxa de conversão e o IOF (passe o mouse no ⓘ para ver a conta).
              Abaixo de cada valor, a variação em relação à coluna anterior. Ativo vendido some das colunas seguintes, porque o dinheiro foi para
              outro lugar (outro ativo ou a conta corrente). A coluna “No período” mostra o resultado, descontando compras e vendas: um ativo
              vendido no meio do ano mostra o que rendeu até a venda. “Desde o início” faz a mesma conta desde a primeira compra de cada investimento. No total geral, é a variação do patrimônio. É um retrato do patrimônio, não da rentabilidade: um aporte aparece como
              alta e um saque como queda. Por isso, embaixo, a linha Rendimento mostra quanto os investimentos renderam sem contar compras e
              vendas. Investimentos são ações, ETFs, criptomoedas, fundos e cofrinhos; FGTS, conta corrente e cartões benefício entram só no total
              geral. A rentabilidade, que desconta os aportes, fica em Carteira › Rentabilidade.
            </>
          }
        >
          Quanto você tinha no fim de cada mês, agrupado por tipo, com o total dos investimentos e o total geral. Para informar o saldo da
          conta, do FGTS, de um cofrinho ou cartão benefício num mês, clique na célula.
        </Intro>
      </div>
      {view.years.length > 0 && year != null && (
        <YearPager years={view.years} year={year} onChange={setYear} />
      )}
    </div>
  );

  if (view.columns.length === 0)
    return (
      <section>
        {head}
        <p className="muted">Sem lançamentos ou saldos para montar a evolução.</p>
      </section>
    );

  const cols = view.columns;
  const noOwned = withoutOwned(view);
  const pairs = comparisons(cols);
  const steps = consecutive(cols.length);
  const order = displayOrder(cols);
  const shown = order.map((i) => ({ c: cols[i], i }));
  const inOrder = <T,>(values: T[]): T[] => order.map((i) => values[i]);

  return (
    <Pairs.Provider value={pairs}>
      <Today.Provider value={cols.findIndex((c) => c.kind === "TODAY")}>
        <section className="evolution">
          {head}
          {view.incomplete && (
            <p className="warning">Falta cotação ou PTAX de algum ativo em alguma data. Use “↻ Atualizar cotações” no topo.</p>
          )}
          <div className="card evo-card">
            <div className="evo-scroll">
              <table className="evo-table">
                <thead>
                  <tr>
                    <th className="evo-name">Em R$</th>
                    {shown.map(({ c }) => {
                      const l = columnLabel(c);
                      return (
                        <th key={c.date} className={`num evo-col ${c.kind === "TODAY" ? "today" : ""}`} title={date(c.date)}>
                          {l.top}
                          {l.sub && <span className="evo-sub">{l.sub}</span>}
                        </th>
                      );
                    })}
                    <th className="num evo-col evo-gain">
                      No período
                      <span className="evo-sub">resultado</span>
                    </th>
                    <th className="num evo-col" title="Resultado desde a primeira compra, com todas as compras e vendas">
                      Desde o início
                      <span className="evo-sub">resultado</span>
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {view.groups.map((g) => {
                    const key = `${g.id}:${g.label}`;
                    // FGTS é um saldo só: a própria linha faz as vezes do grupo.
                    const single = g.id === "fgts" && g.rows.length === 1;
                    const open = single || !collapsed.includes(key);
                    return (
                      <Fragment key={key}>
                        {!single && (
                          <tr className="evo-group clickable" onClick={() => toggle(key)}>
                            <th scope="row">
                              <span className="ret-caret">{open ? "▾" : "▸"}</span>
                              {g.label}
                            </th>
                            {shown.map(({ c, i }) => (
                              <Cell key={c.date} values={g.total} i={i} strong />
                            ))}
                            <Gain gain={g.gain} strong />
                            <Gain gain={g.since} strong plain />
                          </tr>
                        )}
                        {open &&
                          g.rows.map((r) => (
                            <tr key={`${key}:${r.asset ?? r.name}:${r.detail ?? ""}`} className={single ? "evo-row evo-group" : "evo-row"}>
                              <th scope="row" title={[r.name, r.detail, r.value_label].filter(Boolean).join("\n")}>
                                <span className="evo-asset">{r.name}</span>
                                {r.detail && <span className="evo-detail">{r.detail}</span>}
                                {r.value_label && <span className="evo-detail">{r.value_label}</span>}
                              </th>
                              {shown.map(({ c, i }) => (
                                <Cell
                                  key={c.date}
                                  values={r.values}
                                  i={i}
                                  missing={r.missing[i]}
                                  note={r.notes?.[i]}
                                  onEdit={
                                    // 31/12 do ano anterior e "todos os anos" só mostram;
                                    // o saldo se informa no mês dele, dentro do período do saldo.
                                    r.savings_id && !r.outside?.[i] && (c.kind === "MONTH" || c.kind === "TODAY")
                                      ? () =>
                                          setEdit({
                                            savingsId: r.savings_id!,
                                            name: r.name,
                                            date: c.date,
                                            amount:
                                              r.values[i] == null
                                                ? ""
                                                : Number(r.values[i]) === 0
                                                  ? cashZero(2)
                                                  : toCashMask(r.values[i]!, 2).text,
                                            month: c.kind === "MONTH",
                                          })
                                      : undefined
                                  }
                                />
                              ))}
                              <Gain gain={r.gain} />
                              <Gain gain={r.since} plain />
                            </tr>
                          ))}
                      </Fragment>
                    );
                  })}
                </tbody>
                <tfoot>
                  <tr className="evo-total">
                    <th scope="row">Total investimentos</th>
                    {shown.map(({ c, i }) => (
                      <Cell key={c.date} values={view.investments} i={i} strong />
                    ))}
                    <Gain gain={view.investments_gain} strong />
                    <Gain gain={view.investments_since} strong plain />
                  </tr>
                  <FlowRow highlight={order.findIndex((i) => cols[i].kind === "TODAY")} label="Rendimento" hint="Variação dos investimentos sem contar compras e vendas" values={inOrder(incomes(view, pairs))} total={incomes(view, steps)} />
                  {noOwned ? (
                    <>
                      {/* Com bens, a variação vai no total sem eles: os bens entram pelo custo. */}
                      <tr className="evo-total">
                        <th scope="row" title="Total geral sem os imóveis e veículos">
                          Total sem bens
                        </th>
                        {shown.map(({ c, i }) => (
                          <Cell key={c.date} values={noOwned.values} i={i} strong />
                        ))}
                        <Gain gain={noOwned.gain} strong />
                        <td className="num evo-cell" />
                      </tr>
                      <FlowRow highlight={order.findIndex((i) => cols[i].kind === "TODAY")} label="Variação (R$)" hint="Diferença do total sem bens para a coluna anterior" values={inOrder(diffs(noOwned.values, pairs))} total={diffs(noOwned.values, steps)} />
                      <tr className="evo-total grand">
                        <th scope="row">Total geral (com bens)</th>
                        {shown.map(({ c, i }) => (
                          <Cell key={c.date} values={view.total} i={i} strong noRate />
                        ))}
                        <td className="num evo-cell" />
                        <td className="num evo-cell" />
                      </tr>
                    </>
                  ) : (
                    <>
                      <tr className="evo-total grand">
                        <th scope="row">Total geral</th>
                        {shown.map(({ c, i }) => (
                          <Cell key={c.date} values={view.total} i={i} strong />
                        ))}
                        <Gain gain={view.total_gain} strong />
                        <td className="num evo-cell" />
                      </tr>
                      <FlowRow highlight={order.findIndex((i) => cols[i].kind === "TODAY")} label="Variação (R$)" hint="Diferença do total geral para a coluna anterior" values={inOrder(diffs(view.total, pairs))} total={diffs(view.total, steps)} />
                    </>
                  )}
                </tfoot>
              </table>
            </div>
          </div>
          {edit && (
            <BalanceDialog
              edit={edit}
              onClose={() => setEdit(null)}
              onSaved={() => {
                setEdit(null);
                setReload((n) => n + 1);
              }}
            />
          )}
        </section>
      </Today.Provider>
    </Pairs.Provider>
  );
}

type Editing = { savingsId: string; name: string; date: string; amount: string; month: boolean };

/** Saldo informado numa data: grava como "Saldo informado" em Contas e reservas. */
function BalanceDialog({ edit, onClose, onSaved }: { edit: Editing; onClose: () => void; onSaved: () => void }) {
  const [amount, setAmount] = useState(edit.amount);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [confirm, setConfirm] = useState(false);

  // Apaga o saldo informado no mês (FGTS, conta e benefício não têm outra
  // lista de movimentos). Depósitos e resgates de cofrinho ficam.
  async function erase() {
    if (!confirm) return setConfirm(true);
    setBusy(true);
    setError(null);
    try {
      const view = await savingsApi.load();
      const item = view.items.find((x) => x.savings.id === edit.savingsId);
      const month = edit.date.slice(0, 7);
      const saldos = item?.moves.filter((m) => m.kind === "SALDO" && m.date.startsWith(month) && m.date <= edit.date) ?? [];
      if (saldos.length === 0) {
        setBusy(false);
        setConfirm(false);
        return setError("Não há saldo informado neste mês; o valor vem de um depósito ou resgate do cofrinho.");
      }
      for (const m of saldos) await savingsApi.removeMove(m.id);
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  }

  async function save(e: React.FormEvent) {
    e.preventDefault();
    const value = parseDecimalInput(amount);
    if (value == null) return setError("Informe o saldo.");
    setBusy(true);
    setError(null);
    try {
      // Já havia saldo informado nesse dia: corrige em vez de duplicar.
      const view = await savingsApi.load();
      const item = view.items.find((x) => x.savings.id === edit.savingsId);
      const same = item?.moves.find((m) => m.kind === "SALDO" && m.date === edit.date);
      const move = { savings_id: edit.savingsId, date: edit.date, kind: "SALDO" as const, amount: value, ir: null, iof: null, note: null };
      if (same) await savingsApi.updateMove({ ...move, id: same.id });
      else await savingsApi.createMove({ ...move, id: "" });
      onSaved();
    } catch (err) {
      setError(errorMessage(err));
      setBusy(false);
    }
  }

  return (
    <aside className="memo-panel evo-edit" role="dialog" aria-label="Informar saldo">
      <div className="memo-head">
        <h3>
          {edit.name} · {date(edit.date)}
        </h3>
        <button className="link" onClick={onClose} aria-label="Fechar">
          ✕
        </button>
      </div>
      <form onSubmit={save}>
        <label>
          Saldo que o app do banco mostrava nesse dia
          <CashInput value={amount} decimals={2} currency="BRL" onChange={setAmount} required />
        </label>
        {error && <p className="error">{error}</p>}
        <div className="actions">
          <button type="submit" disabled={busy}>
            {busy ? "Salvando…" : "Salvar"}
          </button>
          <button type="button" className="secondary" onClick={onClose}>
            Cancelar
          </button>
          {edit.month && edit.amount !== "" && (
            <>
              <span className="spacer" />
              <button type="button" className={confirm ? "danger" : "link subtle"} disabled={busy} onClick={() => void erase()}>
                {confirm ? "Confirmar: apagar" : "Apagar este saldo"}
              </button>
            </>
          )}
        </div>
      </form>
    </aside>
  );
}
