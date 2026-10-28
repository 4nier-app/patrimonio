import { useEffect, useMemo, useState, type ReactNode } from "react";
import type { EChartsCoreOption } from "echarts/core";
import {
  dashboardApi,
  errorMessage,
  evolutionApi,
  isResgate,
  type DashboardView,
  type Dec,
  type EvolutionGroup,
  type EvolutionView,
  type TaxMonth,
} from "../api";
import Chart, {
  axisStyle,
  base,
  usePalette,
  type Palette,
} from "../charts/Chart";
import { date, money, monthLabel, percent, tone } from "../format";
import { Cards, compact, FirstSteps, type Go } from "./Dashboard";
import { resgateNote } from "./Portfolio";
import { columnLabel, withoutOwned } from "./Evolution";
import YearPager from "../components/YearPager";
import GoalCard from "./GoalCard";

/** Abas e seções que o Painel abre. */
export type PanelGo = (
  tab: "lancamentos" | "carteira" | "seguranca" | "evolucao" | "impostos",
  section?:
    | "manual"
    | "importar"
    | "seguranca"
    | "patrimonio"
    | "saldos"
    | "posicoes"
    | "rentabilidade",
) => void;

/** Fundos de cada banco viram uma fatia só no gráfico e na divisão. */
type Slice = {
  id: string;
  label: string;
  investment: boolean;
  total: (Dec | null)[];
};

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
    bens: "#8a63d2",
  };
  return map[id] ?? p.muted;
}

const num = (v: Dec | null | undefined) => (v == null ? null : Number(v));

function Change({
  label,
  value,
  base,
}: {
  label: string;
  value: number | null;
  base: number | null;
}) {
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

const signed = (v: number) => `${v >= 0 ? "+" : ""}${money(v.toFixed(2))}`;

/**
 * Ponte da variação do ano: rendimento dos investimentos + FGTS + o resto,
 * que é dinheiro novo (salário guardado menos gastos). Fecha com a variação
 * do topo do Painel, para os dois números não parecerem contraditórios.
 */
function Bridge({
  title,
  year,
  total,
  income,
  incomeBase,
  paid,
  fgts,
}: {
  title: string;
  year: number;
  total: number | null;
  income: number | null;
  incomeBase: number | null;
  /** Proventos recebidos no ano, já dentro do rendimento. */
  paid: number;
  /** Variação do FGTS no ano; null quando não há FGTS. */
  fgts: number | null;
}) {
  if (total == null || income == null) {
    return (
      <div className="card stat static bridge-card">
        <span className="stat-label">
          Rendimento dos investimentos em {year}
        </span>
        <span className="stat-value">sem cotação</span>
        <span className="stat-sub">
          Atualize as cotações para ver de onde veio a variação do ano.
        </span>
      </div>
    );
  }
  const rest = total - income - (fgts ?? 0);
  const line = (
    label: string,
    value: number,
    sub: string,
    split?: ReactNode,
  ) => (
    <div className="bridge-line">
      <span>{label}</span>
      <b className={tone(String(value))}>{signed(value)}</b>
      <span className="bridge-sub">{sub}</span>
      {split}
    </div>
  );
  return (
    <div className="card stat static bridge-card">
      <span className="stat-label">{title}</span>
      <div className="bridge">
        {line(
          "Rendimento dos investimentos",
          income,
          `${incomeBase ? `${percent(String(income / incomeBase))} sobre o que estava investido. ` : ""}Compras e vendas não contam, só o quanto os ativos renderam, com os proventos.`,
          paid > 0 ? (
            <span className="bridge-split">
              <span>
                Variação de preço{" "}
                <b className={tone(String(income - paid))}>
                  {signed(income - paid)}
                </b>
              </span>
              <span>
                Proventos <b className="pos">{signed(paid)}</b>
              </span>
            </span>
          ) : undefined,
        )}
        {fgts != null && line("FGTS", fgts, "Depósitos e rendimento do FGTS.")}
        {line(
          "Dinheiro que entrou no seu patrimônio",
          rest,
          rest >= 0
            ? "O que você guardou do salário ou de outras rendas, menos o que gastou ou retirou."
            : "Saiu mais do que entrou: gastos e retiradas passaram do que você guardou.",
        )}
        <div className="bridge-line bridge-total">
          <span>Variação em {year}</span>
          <b className={tone(String(total))}>{signed(total)}</b>
          <span className="bridge-sub">
            O mesmo valor do topo do Painel.
            {income < 0 &&
              total > 0 &&
              " O patrimônio subiu porque entrou dinheiro novo, não porque os investimentos renderam."}
          </span>
        </div>
      </div>
    </div>
  );
}

export default function Panel({
  refresh,
  go,
  hasRecovery,
  darfs,
  simple = false,
}: {
  refresh: number;
  go: PanelGo;
  hasRecovery: boolean;
  darfs: TaxMonth[];
  /** Visão simples: só o total, o rendimento e onde está o dinheiro. */
  simple?: boolean;
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
    if (allYears || !year || chartYear === year.year || byYear[chartYear])
      return;
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
  if (year.columns.length === 0)
    return <FirstSteps go={go as Go} hasRecovery={hasRecovery} />;

  const n = year.columns.length;
  const today = n - 1;
  const total = num(year.total[today]);
  const groups = slices(year.groups);
  const part = (id: string) =>
    num(groups.find((g) => g.id === id)?.total[today]);
  // Fim do último mês fechado (ou 31/12 do ano anterior): base da variação no mês.
  const prevIdx = year.columns.reduce(
    (last, c, i) => (c.kind === "MONTH" || c.kind === "BASE" ? i : last),
    -1,
  );
  // Variações sem os bens: eles entram pelo custo e só achatariam a %.
  const noOwned = withoutOwned(year);
  const liquid = noOwned?.values ?? year.total;
  const liquidGain = noOwned ? noOwned.gain : year.total_gain;
  const liquidNow = num(liquid[today]);
  const prev = prevIdx >= 0 && prevIdx < today ? num(liquid[prevIdx]) : null;
  const yearGain = liquidGain ? Number(liquidGain.value) : null;
  const yearBase = liquidGain ? Number(liquidGain.base) : null;
  const income = year.investments_gain
    ? Number(year.investments_gain.value)
    : null;
  const paid = Number(year.investments_incomes ?? 0);
  const incomeBase = year.investments_gain
    ? Number(year.investments_gain.base)
    : null;
  const thisYear = new Date().getFullYear();
  const fgtsRow = groups.find((g) => g.id === "fgts")?.total;
  const fgtsChange = fgtsRow
    ? Number(fgtsRow[today] ?? 0) - Number(fgtsRow[0] ?? 0)
    : 0;

  // Saldos que não são investimento e ainda não foram informados neste mês.
  const monthStart = new Date();
  monthStart.setDate(1);
  const startIso = monthStart.toISOString().slice(0, 10);
  const stale = (dash.other_balances ?? []).filter(
    (o) => !o.last_informed || o.last_informed < startIso,
  );

  // Corretoras em dólar pelo valor de resgate, com a conta de cada ativo no ⓘ.
  const resgate = dash.positions.filter((p) => isResgate(p.value_fx));
  const resgateAccounts = [...new Set(resgate.flatMap((p) => p.accounts))];
  const resgateTip = resgate
    .map((p) => `${p.asset}: ${resgateNote(p)}`)
    .join("\n\n");

  const alerts = (darfs.length > 0 || stale.length > 0) && (
    <div className="card panel-alerts">
      <h3>Pendências</h3>
      <ul>
        {darfs.map((d) => (
          <li key={d.month}>
            DARF de {money(d.darf)}{" "}
            {d.status === "ATRASADO" ? "venceu" : "vence"} em {date(d.due_date)}
            . É uma estimativa: confira com um contador.{" "}
            <button
              type="button"
              className="link"
              onClick={() => go("impostos")}
            >
              Ver em Impostos ›
            </button>
          </li>
        ))}
        {stale.map((o) => (
          <li key={o.id}>
            Saldo de {o.name}
            {o.account && ` (${o.account})`} ainda não informado em{" "}
            {monthLabel(monthStart.getMonth() + 1)}.{" "}
            <button
              type="button"
              className="link"
              onClick={() => go("evolucao", "patrimonio")}
            >
              Informar na Evolução ›
            </button>
          </li>
        ))}
      </ul>
    </div>
  );

  if (simple) {
    // O mesmo cartão de 12 meses da Rentabilidade; sem 12 meses de histórico, desde o início.
    const card =
      dash.cards.find((c) => c.label === "Últimos 12 meses") ??
      dash.cards.find((c) => c.label === "Desde o início");
    const rate = card?.rate != null ? Number(card.rate) : null;
    const cdi = card?.cdi != null ? Number(card.cdi) : null;
    return (
      <section className="panel">
        <div className="dash-head">
          <div>
            <h2>Painel</h2>
            <p className="muted">
              Quanto você tem e quanto rendeu. Para ver todos os detalhes,
              escolha Visão completa no topo.
            </p>
          </div>
        </div>

        {alerts}

        <div className="card simple-hero">
          <div className="simple-fig">
            <span className="muted">Você tem hoje</span>
            <strong className="simple-big">
              {total == null ? "sem cotação" : money(total.toFixed(2))}
            </strong>
            <span className="muted small">
              somando investimentos, FGTS, contas
              {groups.some((g) => g.id === "bens") ? " e bens" : ""}
            </span>
          </div>
          <div className="simple-fig">
            <span className="muted">
              Seus investimentos renderam em {thisYear}
            </span>
            <strong className={tone(income == null ? null : String(income))}>
              {income == null ? "sem cotação" : money(income.toFixed(2))}
            </strong>
            {income != null && incomeBase ? (
              <span className="muted small">
                {percent(String(income / incomeBase))} sobre o que estava
                investido
                {paid > 0 && `, com ${money(paid.toFixed(2))} de proventos`}
              </span>
            ) : null}
          </div>
          {card && (
            <div className="simple-fig">
              <span className="muted">
                {card.label === "Últimos 12 meses"
                  ? "Nos últimos 12 meses"
                  : "Desde o início"}
              </span>
              <strong className={tone(card.rate)}>{percent(card.rate)}</strong>
              {rate != null && cdi != null && (
                <span className="muted small">
                  {rate >= cdi
                    ? "rendeu mais que o CDI"
                    : "rendeu menos que o CDI"}
                  , que foi de {percent(card.cdi)}
                </span>
              )}
            </div>
          )}
        </div>

        <GoalCard year={year} refresh={refresh} />

        <div className="card dash-block">
          <div className="block-head">
            <h3>Onde está o seu dinheiro</h3>
          </div>
          <Division groups={groups} at={today} total={total} p={p} />
        </div>
      </section>
    );
  }

  return (
    <section className="panel">
      <div className="dash-head">
        <div>
          <h2>Painel</h2>
          <p className="muted">
            Seu patrimônio hoje e como ele chegou até aqui. Valores em reais; o
            que está em dólar vai pela PTAX
            {resgateAccounts.length > 0 && (
              <span title={resgateTip}>
                , e {resgateAccounts.join(", ")} pelo valor de resgate (o que
                chegaria no Brasil) <span className="info-mark">ⓘ</span>
              </span>
            )}
            .
          </p>
        </div>
      </div>

      {alerts}

      <div className="card panel-hero">
        <div className="panel-total">
          <span className="muted">Patrimônio total hoje</span>
          <strong>
            {total == null ? "sem cotação" : money(total.toFixed(2))}
          </strong>
          <div className="panel-changes">
            <Change
              label={noOwned ? "variação no mês, sem bens" : "variação no mês"}
              value={
                liquidNow != null && prev != null ? liquidNow - prev : null
              }
              base={prev}
            />
            <Change
              label={
                noOwned
                  ? `variação em ${thisYear}, sem bens`
                  : `variação em ${thisYear}`
              }
              value={yearGain}
              base={yearBase}
            />
          </div>
        </div>
        <div className="panel-split">
          {[
            { label: "Investimentos", value: num(year.investments[today]) },
            { label: "FGTS", value: part("fgts") },
            { label: "Dinheiro", value: part("dinheiro") },
            ...(groups.some((g) => g.id === "bens")
              ? [{ label: "Bens", value: part("bens") }]
              : []),
          ].map((x) => (
            <div key={x.label}>
              <span className="muted small">{x.label}</span>
              <strong>
                {x.value == null ? "—" : money(x.value.toFixed(2))}
              </strong>
            </div>
          ))}
        </div>
      </div>

      <div className="panel-pair">
        <GoalCard year={year} refresh={refresh} />
        <Bridge
          title={
            noOwned
              ? `De onde veio a variação de ${thisYear}, sem bens`
              : `De onde veio a variação de ${thisYear}`
          }
          year={thisYear}
          total={yearGain}
          income={income}
          incomeBase={incomeBase}
          paid={paid}
          fgts={groups.some((g) => g.id === "fgts") ? fgtsChange : null}
        />
      </div>

      <div className="dash-grid">
        <div className="card dash-block wide">
          <div className="block-head">
            <h3>Evolução do patrimônio</h3>
            <div className="chart-controls">
              {!allYears && (
                <YearPager
                  years={year.years}
                  year={chartYear}
                  onChange={setChartYear}
                  small
                />
              )}
              <div
                className="segmented small"
                role="group"
                aria-label="Período do gráfico"
              >
                <button
                  className={!allYears ? "seg active" : "seg"}
                  onClick={() => setAllYears(false)}
                >
                  Por mês
                </button>
                <button
                  className={allYears ? "seg active" : "seg"}
                  onClick={() => setAllYears(true)}
                >
                  Todos os anos
                </button>
              </div>
            </div>
          </div>
          {(() => {
            const shown = allYears
              ? all
              : chartYear === year.year
                ? year
                : byYear[chartYear];
            return shown ? (
              <Evolution view={shown} p={p} />
            ) : (
              <p className="muted">Calculando…</p>
            );
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
            <button
              type="button"
              className="link"
              onClick={() => go("carteira", "rentabilidade")}
            >
              Ver detalhes ›
            </button>
          </div>
          <p className="muted small">
            Só dos investimentos, com os proventos e descontando aportes e
            resgates, comparada com o CDI.
          </p>
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
      return c.kind === "MONTH" || c.kind === "YEAR"
        ? l.top
        : `${l.top} ${l.sub ?? ""}`.trim();
    });
    return {
      ...base(p),
      tooltip: {
        ...(base(p).tooltip as object),
        trigger: "axis",
        valueFormatter: (v: number) => (v == null ? "—" : money(v.toFixed(2))),
      },
      legend: {
        ...(base(p).legend as object),
        type: "scroll",
        right: 0,
        data: groups.map((g) => g.label),
        pageTextStyle: { color: p.muted },
      },
      xAxis: {
        type: "category",
        data: labels,
        ...axisStyle(p),
        // Todos os meses, sem pular nenhum, com o total completo embaixo do mês.
        axisLabel: {
          color: p.muted,
          interval: 0,
          hideOverlap: false,
          lineHeight: 17,
          formatter: (label: string, i: number) => {
            const v = view.total[i];
            if (v == null) return label;
            const total = Number(v).toLocaleString("pt-BR", {
              minimumFractionDigits: 2,
              maximumFractionDigits: 2,
            });
            return `${label}\n{total|${total}}`;
          },
          rich: { total: { color: p.ink, fontSize: 11, lineHeight: 17 } },
        },
        splitLine: { show: false },
      },
      yAxis: {
        type: "value",
        ...axisStyle(p),
        axisLabel: { color: p.muted, formatter: (v: number) => compact(v) },
      },
      grid: { left: 8, right: 16, top: 40, bottom: 8, containLabel: true },
      series: [
        ...groups.map((g) => ({
          name: g.label,
          type: "bar",
          stack: "p",
          barMaxWidth: 32,
          color: colorOf(g.id, p),
          itemStyle: { borderColor: p.surface, borderWidth: 1 },
          data: g.total.map((v) => (v == null ? null : Number(v))),
        })),
      ],
    };
  }, [view, p]);
  return (
    <Chart
      option={option}
      height={320}
      ariaLabel="Patrimônio no fim de cada período, por grupo"
    />
  );
}

/** Quanto de cada grupo há hoje, em R$ e % do total. */
function Division({
  groups,
  at,
  total,
  p,
}: {
  groups: Slice[];
  at: number;
  total: number | null;
  p: Palette;
}) {
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
              <span
                className="swatch"
                style={{ background: colorOf(r.id, p) }}
              />
              {r.label}
              {!r.investment && (
                <span className="muted small"> · fora dos investimentos</span>
              )}
            </span>
            <span className="num">
              {money(r.value.toFixed(2))}
              {total ? (
                <span className="muted small">
                  {" "}
                  · {percent(String(r.value / total))}
                </span>
              ) : null}
            </span>
          </div>
          <div className="division-bar">
            <span
              style={{
                width: `${Math.max(2, (r.value / max) * 100)}%`,
                background: colorOf(r.id, p),
              }}
            />
          </div>
        </li>
      ))}
    </ul>
  );
}
