import { Fragment, useEffect, useState } from "react";
import { errorMessage, evolutionApi, type Dec, type EvolutionColumn, type EvolutionView } from "../api";
import Intro from "../components/Intro";
import { date, money, monthLabel, percent, tone } from "../format";

// Na tabela, reais sem centavos para caber o ano inteiro; o valor exato fica na dica.
const intFmt = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 0 });
const short = (v: Dec) => intFmt.format(Number(v));

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

function columnLabel(c: EvolutionColumn): { top: string; sub?: string } {
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
    case "FUTURE":
      return { top: monthLabel(Number(m)), sub: `${d}/${m}` };
  }
}

/** Variação simples em relação à coluna anterior: valor ÷ anterior − 1. */
function change(values: (Dec | null)[], i: number): string | null {
  const cur = values[i];
  const prev = i > 0 ? values[i - 1] : null;
  if (cur == null || prev == null || Number(prev) === 0) return null;
  return String(Number(cur) / Number(prev) - 1);
}

/** Do primeiro ao último ponto do período, em R$ e %. */
function periodGain(values: (Dec | null)[], end: number): { gain: string; rate: string | null } | null {
  const first = values[0];
  const last = values[end];
  if (first == null && last == null) return null;
  const a = Number(first ?? 0);
  const b = Number(last ?? 0);
  return { gain: (b - a).toFixed(2), rate: first != null && a !== 0 && last != null ? String(b / a - 1) : null };
}

function Cell({ values, i, missing, strong }: { values: (Dec | null)[]; i: number; missing?: boolean; strong?: boolean }) {
  const v = values[i];
  if (v == null)
    return <td className="num evo-cell">{missing && <span className="ret-missing">sem cotação</span>}</td>;
  const pct = change(values, i);
  return (
    <td className="num evo-cell" title={money(v)}>
      <span className={strong ? "evo-val strong" : "evo-val"}>{short(v)}</span>
      {pct != null && <span className={`evo-pct ${tone(pct)}`}>{percent(pct)}</span>}
    </td>
  );
}

function Gain({ values, end, strong }: { values: (Dec | null)[]; end: number; strong?: boolean }) {
  const g = periodGain(values, end);
  return (
    <td className="num evo-cell evo-gain" title={g ? money(g.gain) : undefined}>
      {g && (
        <>
          <span className={`evo-val ${strong ? "strong" : ""} ${tone(g.gain)}`}>{short(g.gain)}</span>
          {g.rate != null && <span className={`evo-pct ${tone(g.rate)}`}>{percent(g.rate)}</span>}
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
    </tr>
  );
}

/** Diferença para a coluna anterior, até a coluna `end` (hoje). */
const diffs = (values: (Dec | null)[], end: number): (string | null)[] =>
  values.map((v, i) => {
    if (i === 0 || i > end) return null;
    const prev = values[i - 1];
    if (v == null && prev == null) return null;
    return (Number(v ?? 0) - Number(prev ?? 0)).toFixed(2);
  });

export default function Evolution() {
  const [year, setYear] = useState<number | null | undefined>(undefined);
  const [view, setView] = useState<EvolutionView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [collapsed, setCollapsed] = useState<string[]>(loadCollapsed);

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
  }, [year]);

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
              Cada coluna é o patrimônio no fim do mês (ou do ano, em “Todos os anos”), em reais; o que está em dólar é convertido pela PTAX do dia.
              Abaixo de cada valor, a variação em relação à coluna anterior. É um retrato do patrimônio, não da rentabilidade: um aporte aparece como
              alta e um saque como queda. Por isso, embaixo, a variação dos investimentos é separada em dinheiro novo (aportes menos resgates) e
              rendimento. Investimentos são ações, ETFs, criptomoedas, fundos e cofrinhos; FGTS, conta corrente e cartões benefício entram só no total
              geral. A rentabilidade, que desconta os aportes, continua na aba Rentabilidade.
            </>
          }
        >
          Quanto você tinha no fim de cada mês, agrupado por tipo, com o total dos investimentos e o total geral.
        </Intro>
      </div>
      {view.years.length > 0 && (
        <label className="ret-pick">
          Período
          <select value={year == null ? "todos" : String(year)} onChange={(e) => setYear(e.target.value === "todos" ? null : Number(e.target.value))}>
            {view.years.map((y) => (
              <option key={y} value={y}>
                {y}
              </option>
            ))}
            <option value="todos">Todos os anos</option>
          </select>
        </label>
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
  // "No período" vai até hoje: meses futuros ficam fora.
  const end = cols.reduce((last, c, i) => (c.kind === "FUTURE" ? last : i), 0);
  const investDiffs = diffs(view.investments, end);
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
                    <th key={c.date} className={`num evo-col ${c.kind === "TODAY" ? "today" : c.kind === "FUTURE" ? "future" : ""}`} title={date(c.date)}>
                      {l.top}
                      {l.sub && <span className="evo-sub">{l.sub}</span>}
                    </th>
                  );
                })}
                <th className="num evo-col evo-gain">
                  No período
                  <span className="evo-sub">R$ e %</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {view.groups.map((g) => {
                const key = `${g.id}:${g.label}`;
                const open = !collapsed.includes(key);
                return (
                  <Fragment key={key}>
                    <tr className="evo-group clickable" onClick={() => toggle(key)}>
                      <th scope="row">
                        <span className="ret-caret">{open ? "▾" : "▸"}</span>
                        {g.label}
                      </th>
                      {cols.map((c, i) => (
                        <Cell key={c.date} values={g.total} i={i} strong />
                      ))}
                      <Gain values={g.total} end={end} strong />
                    </tr>
                    {open &&
                      g.rows.map((r) => (
                        <tr key={`${key}:${r.asset ?? r.name}:${r.detail ?? ""}`} className="evo-row">
                          <th scope="row">
                            <span className="evo-asset">{r.name}</span>
                            {r.detail && <span className="evo-detail">{r.detail}</span>}
                          </th>
                          {cols.map((c, i) => (
                            <Cell key={c.date} values={r.values} i={i} missing={r.missing[i]} />
                          ))}
                          <Gain values={r.values} end={end} />
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
                <Gain values={view.investments} end={end} strong />
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
                <Gain values={view.total} end={end} strong />
              </tr>
              <FlowRow label="Variação (R$)" hint="Diferença do total geral para a coluna anterior" values={diffs(view.total, end)} />
            </tfoot>
          </table>
        </div>
      </div>
    </section>
  );
}
