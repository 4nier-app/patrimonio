import { useEffect, useMemo, useState } from "react";
import type { EChartsCoreOption } from "echarts/core";
import {
  dashboardApi,
  errorMessage,
  evolutionApi,
  type DashboardView,
  type Dec,
  type EvolutionGroup,
  type EvolutionView,
  type TaxMonth,
} from "../api";
import Chart, { axisStyle, base, usePalette, type Palette } from "../charts/Chart";
import { date, money, monthLabel, percent, tone } from "../format";
import { Cards, compact, FirstSteps, type Go } from "./Dashboard";
import { columnLabel } from "./Evolution";
import YearPager from "../components/YearPager";

/** Abas e seções que o Painel abre. */
export type PanelGo = (
  tab: "lancamentos" | "carteira" | "seguranca" | "evolucao" | "impostos",
  section?: "manual" | "importar" | "seguranca" | "patrimonio" | "saldos" | "posicoes" | "rentabilidade",
) => void;

/** Fundos de cada banco viram uma fatia só no gráfico e na divisão. */
type Slice = { id: string; label: string; investment: boolean; total: (Dec | null)[] };

function slices(groups: EvolutionGroup[]): Slice[] {
  const out: Slice[] = [];
  for (const g of groups) {
    const id = g.id;
    const label = id === "fundos" ? "Fundos de investimento" : g.label;
    const found = out.find((s) => s.id === id);
    if (!found) {
      out.push({ id, label, investment: g.investment, total: [...g.total] });
      continue;
    }
    found.total = found.total.map((v, i) => {
      const w = g.total[i];
      if (v == null) return w;
      if (w == null) return v;
      return (Number(v) + Number(w)).toFixed(2);
    });
  }
  return out;
}

/** Cor fixa por grupo, a mesma no gráfico e na divisão. */
function colorOf(id: string, p: Palette): string {
  const map: Record<string, string> = {
    fundos: p.series[0],
    b3: p.series[1],
    fii: p.series[4],
    exterior: p.series[2],
    cripto: p.series[3],
    rf: p.series[5],
    fgts: p.muted,
    dinheiro: p.axis,
  };
  return map[id] ?? p.muted;
}

const num = (v: Dec | null | undefined) => (v == null ? null : Number(v));

function Change({ label, value, base }: { label: string; value: number | null; base: number | null }) {
  if (value == null) return null;
  const rate = base ? value / base : null;
  return (
    <span className="panel-change">
      <span className="muted small">{label}</span>
      <span className={tone(String(value))}>
        {value >= 0 ? "+" : ""}
        {money(value.toFixed(2))}
        {rate != null && ` (${percent(String(rate))})`}
      </span>
    </span>
  );
}

export default function Panel({
  refresh,
  go,
  hasRecovery,
  darfs,
}: {
  refresh: number;
  go: PanelGo;
  hasRecovery: boolean;
  darfs: TaxMonth[];
}) {
  const [year, setYear] = useState<EvolutionView | null>(null);
  const [all, setAll] = useState<EvolutionView | null>(null);
  const [dash, setDash] = useState<DashboardView | null>(null);
  const [allYears, setAllYears] = useState(false);
  // Ano do gráfico; os outros anos são carregados quando escolhidos.
  const [chartYear, setChartYear] = useState(() => new Date().getFullYear());
  const [byYear, setByYear] = useState<Record<number, EvolutionView>>({});
  const [error, setError] = useState<string | null>(null);
  const p = usePalette();

  useEffect(() => {
    const now = new Date().getFullYear();
    Promise.all([evolutionApi.load(now), dashboardApi.load()])
      .then(([e, d]) => {
        setYear(e);
        setDash(d);
      })
      .catch((e) => setError(errorMessage(e)));
  }, [refresh]);

  useEffect(() => {
    setAll(null);
    setByYear({});
  }, [refresh]);

  useEffect(() => {
    if (allYears || !year || chartYear === year.year || byYear[chartYear]) return;
    evolutionApi
      .load(chartYear)
      .then((v) => setByYear((m) => ({ ...m, [chartYear]: v })))
      .catch((e) => setError(errorMessage(e)));
  }, [allYears, chartYear, year, byYear]);

  useEffect(() => {
    if (!allYears || all) return;
    evolutionApi
      .load(null)
      .then(setAll)
      .catch((e) => setError(errorMessage(e)));
  }, [allYears, all]);

  if (error) return <p className="error">{error}</p>;
  if (!year || !dash) return <p className="muted">Calculando…</p>;
  if (year.columns.length === 0) return <FirstSteps go={go as Go} hasRecovery={hasRecovery} />;

  const n = year.columns.length;
  const today = n - 1;
  const total = num(year.total[today]);
  const groups = slices(year.groups);
  const part = (id: string) => num(groups.find((g) => g.id === id)?.total[today]);
  // Fim do último mês fechado (ou 31/12 do ano anterior): base da variação no mês.
  const prevIdx = year.columns.reduce((last, c, i) => (c.kind === "MONTH" || c.kind === "BASE" ? i : last), -1);
  const prev = prevIdx >= 0 && prevIdx < today ? num(year.total[prevIdx]) : null;
  const yearGain = year.total_gain ? Number(year.total_gain.value) : null;
  const yearBase = year.total_gain ? Number(year.total_gain.base) : null;
  const newMoney = year.new_money.reduce((t, v) => t + Number(v ?? 0), 0);
  const income = year.investments_gain ? Number(year.investments_gain.value) : null;
  const incomeBase = year.investments_gain ? Number(year.investments_gain.base) : null;
  const thisYear = new Date().getFullYear();

  // Saldos que não são investimento e ainda não foram informados neste mês.
  const monthStart = new Date();
  monthStart.setDate(1);
  const startIso = monthStart.toISOString().slice(0, 10);
  const stale = (dash.other_balances ?? []).filter((o) => !o.last_informed || o.last_informed < startIso);

  return (
    <section className="panel">
      <div className="dash-head">
        <div>
          <h2>Painel</h2>
          <p className="muted">Seu patrimônio hoje e como ele chegou até aqui. Valores em reais; o que está em dólar vai pela PTAX.</p>
        </div>
      </div>

      {(darfs.length > 0 || stale.length > 0) && (
        <div className="card panel-alerts">
          <h3>Pendências</h3>
          <ul>
            {darfs.map((d) => (
              <li key={d.month}>
                DARF de {money(d.darf)} {d.status === "ATRASADO" ? "venceu" : "vence"} em {date(d.due_date)}.{" "}
                <button type="button" className="link" onClick={() => go("impostos")}>
                  Ver em Impostos ›
                </button>
              </li>
            ))}
            {stale.map((o) => (
              <li key={o.id}>
                Saldo de {o.name} ({o.account}) ainda não informado em {monthLabel(monthStart.getMonth() + 1)}.{" "}
                <button type="button" className="link" onClick={() => go("evolucao", "patrimonio")}>
                  Informar na Evolução ›
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="card panel-hero">
        <div className="panel-total">
          <span className="muted">Patrimônio total hoje</span>
          <strong>{total == null ? "sem cotação" : money(total.toFixed(2))}</strong>
          <div className="panel-changes">
            <Change label="no mês" value={total != null && prev != null ? total - prev : null} base={prev} />
            <Change label={`em ${thisYear}`} value={yearGain} base={yearBase} />
          </div>
        </div>
        <div className="panel-split">
          {[
            { label: "Investimentos", value: num(year.investments[today]) },
            { label: "FGTS", value: part("fgts") },
            { label: "Dinheiro", value: part("dinheiro") },
          ].map((x) => (
            <div key={x.label}>
              <span className="muted small">{x.label}</span>
              <strong>{x.value == null ? "—" : money(x.value.toFixed(2))}</strong>
            </div>
          ))}
        </div>
      </div>

      <div className="dash-cards">
        <div className="card stat static">
          <span className="stat-label">Dinheiro novo em {thisYear}</span>
          <span className={`stat-value ${tone(String(newMoney))}`}>{money(newMoney.toFixed(2))}</span>
          <span className="stat-sub">Aportes menos resgates nos investimentos</span>
        </div>
        <div className="card stat static">
          <span className="stat-label">Rendimento em {thisYear}</span>
          <span className={`stat-value ${tone(income == null ? null : String(income))}`}>
            {income == null ? "sem cotação" : money(income.toFixed(2))}
          </span>
          <span className="stat-sub">
            {income != null && incomeBase ? `${percent(String(income / incomeBase))} sobre o que estava investido` : "O que os investimentos renderam"}
          </span>
        </div>
      </div>

      <div className="dash-grid">
        <div className="card dash-block wide">
          <div className="block-head">
            <h3>Evolução do patrimônio</h3>
            <div className="chart-controls">
              {!allYears && <YearPager years={year.years} year={chartYear} onChange={setChartYear} small />}
              <div className="segmented small" role="group" aria-label="Período do gráfico">
                <button className={!allYears ? "seg active" : "seg"} onClick={() => setAllYears(false)}>
                  Por mês
                </button>
                <button className={allYears ? "seg active" : "seg"} onClick={() => setAllYears(true)}>
                  Todos os anos
                </button>
              </div>
            </div>
          </div>
          {(() => {
            const shown = allYears ? all : chartYear === year.year ? year : byYear[chartYear];
            return shown ? <Evolution view={shown} p={p} /> : <p className="muted">Calculando…</p>;
          })()}
        </div>

        <div className="card dash-block">
          <div className="block-head">
            <h3>Onde está o patrimônio</h3>
          </div>
          <Division groups={groups} at={today} total={total} p={p} />
        </div>

        <div className="card dash-block">
          <div className="block-head">
            <h3>Rentabilidade</h3>
            <button type="button" className="link" onClick={() => go("carteira", "rentabilidade")}>
              Ver detalhes ›
            </button>
          </div>
          <p className="muted small">Só dos investimentos, descontando aportes e resgates, comparada com o CDI.</p>
          <div className="panel-returns">
            <Cards view={dash} />
          </div>
        </div>
      </div>
    </section>
  );
}

/** Barras empilhadas por grupo, uma por coluna da Evolução. */
function Evolution({ view, p }: { view: EvolutionView; p: Palette }) {
  const option = useMemo<EChartsCoreOption>(() => {
    const groups = slices(view.groups);
    const labels = view.columns.map((c) => {
      const l = columnLabel(c);
      return c.kind === "MONTH" || c.kind === "YEAR" ? l.top : `${l.top} ${l.sub ?? ""}`.trim();
    });
    return {
      ...base(p),
      tooltip: {
        ...(base(p).tooltip as object),
        trigger: "axis",
        valueFormatter: (v: number) => (v == null ? "—" : money(v.toFixed(2))),
      },
      legend: { ...(base(p).legend as object), type: "scroll", right: 0, data: groups.map((g) => g.label), pageTextStyle: { color: p.muted } },
      xAxis: { type: "category", data: labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      grid: { left: 8, right: 16, top: 40, bottom: 8, containLabel: true },
      series: groups.map((g) => ({
        name: g.label,
        type: "bar",
        stack: "p",
        barMaxWidth: 32,
        color: colorOf(g.id, p),
        itemStyle: { borderColor: p.surface, borderWidth: 1 },
        data: g.total.map((v) => (v == null ? null : Number(v))),
      })),
    };
  }, [view, p]);
  return <Chart option={option} height={320} ariaLabel="Patrimônio no fim de cada período, por grupo" />;
}

/** Quanto de cada grupo há hoje, em R$ e % do total. */
function Division({ groups, at, total, p }: { groups: Slice[]; at: number; total: number | null; p: Palette }) {
  const rows = groups
    .map((g) => ({ ...g, value: num(g.total[at]) ?? 0 }))
    .filter((g) => g.value !== 0)
    .sort((a, b) => b.value - a.value);
  if (rows.length === 0) return <p className="muted">Nada informado ainda.</p>;
  const max = Math.max(...rows.map((r) => r.value));
  return (
    <ul className="division">
      {rows.map((r) => (
        <li key={r.id}>
          <div className="division-head">
            <span>
              <span className="swatch" style={{ background: colorOf(r.id, p) }} />
              {r.label}
              {!r.investment && <span className="muted small"> · fora dos investimentos</span>}
            </span>
            <span className="num">
              {money(r.value.toFixed(2))}
              {total ? <span className="muted small"> · {percent(String(r.value / total))}</span> : null}
            </span>
          </div>
          <div className="division-bar">
            <span style={{ width: `${Math.max(2, (r.value / max) * 100)}%`, background: colorOf(r.id, p) }} />
          </div>
        </li>
      ))}
    </ul>
  );
}
