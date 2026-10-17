import { Fragment, useEffect, useState } from "react";
import YearPager from "../components/YearPager";
import { errorMessage, evolutionApi, savingsApi, type Dec, type EvolutionColumn, type EvolutionGain, type EvolutionView } from "../api";
import CashInput from "../components/CashInput";
import Intro from "../components/Intro";
import { date, money, monthLabel, parseDecimalInput, percent, toCashMask, tone } from "../format";

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
function change(values: (Dec | null)[], i: number): string | null {
  const cur = values[i];
  const prev = i > 0 ? values[i - 1] : null;
  if (cur == null || prev == null || Number(prev) === 0) return null;
  return String(Number(cur) / Number(prev) - 1);
}

function Cell({
  values,
  i,
  missing,
  strong,
  onEdit,
}: {
  values: (Dec | null)[];
  i: number;
  missing?: boolean;
  strong?: boolean;
  /** Saldo: clicar abre o campo para informar o valor nessa data. */
  onEdit?: () => void;
}) {
  const v = values[i];
  const edit = onEdit ? { className: "num evo-cell editable", onClick: onEdit, title: `${v == null ? "" : `${money(v)} · `}clique para informar o saldo` } : null;
  if (v == null)
    return edit ? (
      <td {...edit}>
        <span className="evo-add">+</span>
      </td>
    ) : (
      <td className="num evo-cell">{missing && <span className="ret-missing">sem cotação</span>}</td>
    );
  const pct = change(values, i);
  return (
    <td className={edit?.className ?? "num evo-cell"} title={edit?.title ?? money(v)} onClick={onEdit}>
      <span className={strong ? "evo-val strong" : "evo-val"}>{short(v)}</span>
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

/** Linha só de valores em R$ (variação, dinheiro novo, rendimento), sem %. */
function FlowRow({ label, hint, values }: { label: string; hint: string; values: (string | null)[] }) {
  const known = values.filter((v): v is string => v != null);
  const sum = known.length ? known.reduce((s, v) => s + Number(v), 0).toFixed(2) : null;
  return (
    <tr className="evo-flow">
      <th scope="row" title={hint}>
        {label}
      </th>
      {values.map((v, i) => (
        <td key={i} className="num evo-cell" title={v == null ? undefined : money(v)}>
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
const diffs = (values: (Dec | null)[]): (string | null)[] =>
  values.map((v, i) => {
    if (i === 0) return null;
    const prev = values[i - 1];
    if (v == null && prev == null) return null;
    return (Number(v ?? 0) - Number(prev ?? 0)).toFixed(2);
  });

export default function Evolution({ onSavings }: { onSavings?: () => void }) {
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
              Cada coluna é o patrimônio no fim do mês, em reais; o que está em dólar é convertido pela PTAX do dia.
              Abaixo de cada valor, a variação em relação à coluna anterior. Ativo vendido some das colunas seguintes, porque o dinheiro foi para
              outro lugar (outro ativo ou a conta corrente). A coluna “No período” mostra o resultado, descontando compras e vendas: um ativo
              vendido no meio do ano mostra o que rendeu até a venda. “Desde o início” faz a mesma conta desde a primeira compra de cada investimento. No total geral, é a variação do patrimônio. É um retrato do patrimônio, não da rentabilidade: um aporte aparece como
              alta e um saque como queda. Por isso, embaixo, a variação dos investimentos é separada em dinheiro novo (aportes menos resgates) e
              rendimento. Investimentos são ações, ETFs, criptomoedas, fundos e cofrinhos; FGTS, conta corrente e cartões benefício entram só no total
              geral. A rentabilidade, que desconta os aportes, fica em Carteira › Rentabilidade.
            </>
          }
        >
          Quanto você tinha no fim de cada mês, agrupado por tipo, com o total dos investimentos e o total geral. Para informar o saldo da
          conta, do FGTS, de um cofrinho ou cartão benefício num mês, clique na célula.
          {onSavings && (
            <>
              {" "}
              <button type="button" className="link" onClick={onSavings}>
                Cadastrar conta, FGTS ou cofrinho ›
              </button>
            </>
          )}
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
  const investDiffs = diffs(view.investments);
  const income = investDiffs.map((d, i) => {
    const n = view.new_money[i];
    return d == null || n == null ? null : (Number(d) - Number(n)).toFixed(2);
  });

  return (
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
                {cols.map((c) => {
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
                        {cols.map((c, i) => (
                          <Cell key={c.date} values={g.total} i={i} strong />
                        ))}
                        <Gain gain={g.gain} strong />
                        <Gain gain={g.since} strong plain />
                      </tr>
                    )}
                    {open &&
                      g.rows.map((r) => (
                        <tr key={`${key}:${r.asset ?? r.name}:${r.detail ?? ""}`} className={single ? "evo-row evo-group" : "evo-row"}>
                          <th scope="row" title={r.detail ? `${r.name}\n${r.detail}` : r.name}>
                            <span className="evo-asset">{r.name}</span>
                            {r.detail && <span className="evo-detail">{r.detail}</span>}
                          </th>
                          {cols.map((c, i) => (
                            <Cell
                              key={c.date}
                              values={r.values}
                              i={i}
                              missing={r.missing[i]}
                              onEdit={
                                // 31/12 do ano anterior e "todos os anos" só mostram;
                                // o saldo se informa no mês dele, dentro do período do saldo.
                                r.savings_id && !r.outside?.[i] && (c.kind === "MONTH" || c.kind === "TODAY")
                                  ? () =>
                                      setEdit({
                                        savingsId: r.savings_id!,
                                        name: r.name,
                                        date: c.date,
                                        amount: r.values[i] == null ? "" : toCashMask(r.values[i]!, 2).text,
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
                {cols.map((c, i) => (
                  <Cell key={c.date} values={view.investments} i={i} strong />
                ))}
                <Gain gain={view.investments_gain} strong />
                <Gain gain={view.investments_since} strong plain />
              </tr>
              <FlowRow
                label="Dinheiro novo"
                hint="Aportes menos resgates nos investimentos desde a coluna anterior"
                values={view.new_money.map((v, i) => (i === 0 ? null : v))}
              />
              <FlowRow label="Rendimento" hint="Variação dos investimentos menos o dinheiro novo" values={income} />
              <tr className="evo-total grand">
                <th scope="row">Total geral</th>
                {cols.map((c, i) => (
                  <Cell key={c.date} values={view.total} i={i} strong />
                ))}
                <Gain gain={view.total_gain} strong />
                <td className="num evo-cell" />
              </tr>
              <FlowRow label="Variação (R$)" hint="Diferença do total geral para a coluna anterior" values={diffs(view.total)} />
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
  );
}

type Editing = { savingsId: string; name: string; date: string; amount: string; month: boolean };

/** Saldo informado numa data: grava como "Saldo informado" em Saldos e cofrinhos. */
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
