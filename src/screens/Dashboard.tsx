import { assetLabel } from "../currency";
import { useEffect, useMemo, useState } from "react";
import type { EChartsCoreOption } from "echarts/core";
import { dashboardApi, errorMessage, type DashboardView, type DaysView } from "../api";
import Chart, { axisStyle, base, usePalette, type Palette } from "../charts/Chart";
import {
  cumulative,
  dayFrame,
  monthFrame,
  monthKey,
  monthName,
  PERIODS,
  profitsByYear,
  singleMonth,
  type Frame,
  type Period,
} from "../dashboard";
import { date, money, percent } from "../format";

type Memo = { title: string; lines: string[] };

const brl = (n: number) => money(String(n));
const pct = (n: number | null) => (n == null ? "—" : percent(String(n)));

export type Go = (tab: "lancamentos" | "carteira" | "seguranca", section?: "manual" | "importar" | "seguranca") => void;

/** Primeiros passos, enquanto o cofre ainda não tem lançamentos. */
function FirstSteps({ go, hasRecovery }: { go: Go; hasRecovery: boolean }) {
  return (
    <section className="first-steps">
      <h2>Bem-vindo ao 4Nier</h2>
      <p className="muted">Três passos para ver sua carteira e a rentabilidade de verdade:</p>
      <ol className="steps">
        <li className="card">
          <strong>Traga suas operações</strong>
          <span className="muted">Importe os PDFs das notas de corretagem ou lance compras e vendas à mão.</span>
          <div className="actions">
            <button type="button" onClick={() => go("lancamentos", "importar")}>
              Importar notas
            </button>
            <button type="button" className="secondary" onClick={() => go("lancamentos", "manual")}>
              Lançar à mão
            </button>
          </div>
        </li>
        <li className="card">
          <strong>Confira a Carteira</strong>
          <span className="muted">Veja quantidade, preço médio e resultado de cada ativo. Clique num ativo para ver o gráfico.</span>
          <div className="actions">
            <button type="button" className="secondary" onClick={() => go("carteira")}>
              Abrir a Carteira
            </button>
          </div>
        </li>
        <li className="card">
          <strong>Acompanhe aqui no Painel</strong>
          <span className="muted">Com as operações lançadas, esta tela mostra a rentabilidade comparada com CDI, IPCA e Ibovespa.</span>
        </li>
      </ol>
      {!hasRecovery && (
        <p className="muted small">
          Dica: crie a chave de recuperação em{" "}
          <button type="button" className="link" onClick={() => go("seguranca", "seguranca")}>
            Segurança
          </button>
          . Sem ela, se você esquecer a senha, não há como abrir os dados.
        </p>
      )}
    </section>
  );
}

export default function Dashboard({ refresh, go, hasRecovery }: { refresh: number; go: Go; hasRecovery: boolean }) {
  const [view, setView] = useState<DashboardView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [period, setPeriod] = useState<Period>("inicio");
  const [byYear, setByYear] = useState(false);
  const [inMoney, setInMoney] = useState(false);
  const [memo, setMemo] = useState<Memo | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const [days, setDays] = useState<{ key: string; view: DaysView } | null>(null);
  const p = usePalette();
  const one = singleMonth(period);
  const oneKey = one ? `${monthKey(one)}#${refresh}` : null;

  useEffect(() => {
    dashboardApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }, [refresh]);

  // Um mês só: busca os dias dele.
  useEffect(() => {
    if (!one || !oneKey) return;
    let live = true;
    dashboardApi
      .days(one.year, one.month)
      .then((v) => live && setDays({ key: oneKey, view: v }))
      .catch((e) => live && setError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [oneKey]);

  const frame = useMemo<Frame | null>(() => {
    if (!view) return null;
    if (oneKey) return days?.key === oneKey ? dayFrame(days.view) : null;
    return monthFrame(view, period);
  }, [view, period, oneKey, days]);

  if (error) return <p className="error">{error}</p>;
  if (!view) return <p className="muted">Calculando…</p>;
  if (view.months.length === 0) return <FirstSteps go={go} hasRecovery={hasRecovery} />;

  const years = [...new Set(view.months.map((m) => m.year))];

  return (
    <section className="dashboard">
      <div className="dash-head">
        <div>
          <h2>Painel</h2>
          <p className="muted">
            Clique num cartão para ver a memória de cálculo. Valores em reais; exterior pela PTAX.
          </p>
        </div>
        <div className="period-pick">
          <div className="segmented" role="group" aria-label="Período">
            {PERIODS.map((x) => (
              <button key={x.id} className={period === x.id ? "seg active" : "seg"} onClick={() => setPeriod(x.id)}>
                {x.label}
              </button>
            ))}
            <button
              className={typeof period === "object" && "year" in period ? "seg active" : "seg"}
              onClick={() => {
                if (typeof period !== "object" || !("year" in period)) setPeriod({ year: years[years.length - 1] });
              }}
            >
              Escolher ano
            </button>
            <button
              className={typeof period === "object" && "from" in period ? "seg active" : "seg"}
              onClick={() => {
                if (typeof period === "object" && "from" in period) return;
                const last = monthKey(view.months[view.months.length - 1]);
                setPeriod({ from: last, to: last });
              }}
            >
              Escolher meses
            </button>
          </div>
          {typeof period === "object" && "year" in period && (
            <div className="month-range">
              <label>
                Ano
                <select value={period.year} onChange={(e) => setPeriod({ year: Number(e.target.value) })}>
                  {years.map((y) => (
                    <option key={y} value={y}>
                      {y}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
          {typeof period === "object" && "from" in period && (
            <div className="month-range">
              <label>
                De
                <select value={period.from} onChange={(e) => setPeriod({ ...period, from: e.target.value })}>
                  {view.months.map((m) => (
                    <option key={monthKey(m)} value={monthKey(m)}>
                      {monthName(m)}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Até
                <select value={period.to} onChange={(e) => setPeriod({ ...period, to: e.target.value })}>
                  {view.months.map((m) => (
                    <option key={monthKey(m)} value={monthKey(m)}>
                      {monthName(m)}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
        </div>
      </div>

      <Cards view={view} onMemo={setMemo} />

      {!frame ? (
        <p className="muted">Calculando os dias do mês…</p>
      ) : (
        <div className="dash-grid">
          <Block
            title={frame.days ? "Comparação com índices, dia a dia" : "Comparação com índices"}
            wide
            extra={
              <div className="segmented small" role="group" aria-label="Mostrar em">
                <button className={!inMoney ? "seg active" : "seg"} onClick={() => setInMoney(false)}>
                  %
                </button>
                <button className={inMoney ? "seg active" : "seg"} onClick={() => setInMoney(true)}>
                  R$
                </button>
              </div>
            }
          >
            <p className="muted small">
              {inMoney
                ? "Quanto você teria se cada aporte tivesse ido para o índice, na mesma data e no mesmo valor."
                : "Quanto cada real rendeu, sem depender de quando ou quanto você aportou."}
            </p>
            {inMoney ? <SameFlows frame={frame} p={p} /> : <Comparison frame={frame} p={p} />}
          </Block>
          <Block title="Evolução do patrimônio">
            <Patrimony frame={frame} p={p} />
          </Block>
          <Block
            title="Lucro / prejuízo em R$"
            extra={
              frame.days ? undefined : (
                <div className="segmented small" role="group" aria-label="Agrupar">
                  <button className={!byYear ? "seg active" : "seg"} onClick={() => setByYear(false)}>
                    Mês
                  </button>
                  <button className={byYear ? "seg active" : "seg"} onClick={() => setByYear(true)}>
                    Ano
                  </button>
                </div>
              )
            }
          >
            <Profit frame={frame} byYear={byYear && !frame.days} p={p} />
          </Block>
          <Block title="O que puxou o resultado">
            <Contributions frame={frame} p={p} names={view.names} />
          </Block>
          <Block title="Ativos na carteira hoje">
            <Allocation view={view} p={p} open={open} setOpen={setOpen} />
          </Block>
          <Block title="Queda desde o pico">
            <Drawdown frame={frame} p={p} />
          </Block>
          <Block title="Realizado × não realizado">
            <Realized view={view} p={p} />
          </Block>
          <Block title="Linha do tempo das posições" wide>
            <Timeline view={view} />
          </Block>
        </div>
      )}

      {memo && (
        <aside className="memo-panel" role="dialog" aria-label="Memória de cálculo">
          <div className="memo-head">
            <h3>{memo.title}</h3>
            <button className="link" onClick={() => setMemo(null)} aria-label="Fechar">
              ✕
            </button>
          </div>
          <ul className="memo-lines">
            {memo.lines.map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ul>
        </aside>
      )}
    </section>
  );
}

function Block({ title, wide, extra, children }: { title: string; wide?: boolean; extra?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className={wide ? "card dash-block wide" : "card dash-block"}>
      <div className="block-head">
        <h3>{title}</h3>
        {extra}
      </div>
      {children}
    </div>
  );
}

function Cards({ view, onMemo }: { view: DashboardView; onMemo: (m: Memo) => void }) {
  return (
    <div className="dash-cards">
      {view.cards.map((c) => {
        const diff = c.rate != null && c.cdi != null ? Number(c.rate) - Number(c.cdi) : null;
        return (
          <button
            key={c.label}
            className="card stat"
            onClick={() =>
              onMemo({ title: `${c.label} (${date(c.from)} a ${date(c.to)})`, lines: c.explanation })
            }
          >
            <span className="stat-label">{c.label}</span>
            <span className={`stat-value ${c.rate == null ? "" : Number(c.rate) >= 0 ? "pos" : "neg"}`}>
              {percent(c.rate)}
            </span>
            <span className="stat-sub">
              CDI {percent(c.cdi)}
              {diff != null &&
                ` · ${Math.abs(diff * 100).toLocaleString("pt-BR", { maximumFractionDigits: 2 })} p.p. ${diff >= 0 ? "acima" : "abaixo"}`}
            </span>
          </button>
        );
      })}
    </div>
  );
}

/** Linhas atravessam os meses sem carteira (ano com 12 meses). */
const line = { type: "line", connectNulls: true } as const;

function Comparison({ frame, p }: { frame: Frame; p: Palette }) {
  const option = useMemo<EChartsCoreOption>(() => {
    // Toda linha parte de 0% no início do período, para haver linha mesmo com um ponto só.
    const fromZero = (c: (number | null)[]) => (c.every((v) => v == null) ? [null, ...c] : [0, ...c]);
    const series = [
      { name: "Carteira", data: fromZero(cumulative(frame.slots.map((m) => (m ? m.rate : undefined)))) },
      ...frame.bench.map((bm) => ({ name: bm.name, data: fromZero(cumulative(bm.rates)) })),
    ];
    return {
      ...base(p),
      color: p.series,
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => pct(v) },
      xAxis: { type: "category", data: ["início", ...frame.labels], boundaryGap: false, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => pct(v) } },
      series: series.map((s, i) => ({
        ...line,
        name: s.name,
        data: s.data,
        showSymbol: !frame.days,
        symbolSize: 8,
        lineStyle: { width: i === 0 ? 3 : 2 },
        endLabel: { show: true, color: p.ink, formatter: (x: { value: number }) => pct(x.value) },
        emphasis: { focus: "series" },
      })),
      grid: { left: 8, right: 64, top: 40, bottom: 8, containLabel: true },
    };
  }, [frame, p]);
  const missing = frame.bench.filter((bm) => bm.rates.every((r) => r == null)).map((bm) => bm.name);
  return (
    <>
      <Chart option={option} height={320} ariaLabel="Rentabilidade acumulada da carteira e dos índices" />
      <p className="muted small">
        Acumulado no período, com os índices medidos nos mesmos dias em que havia dinheiro na carteira. Clique no nome
        para mostrar ou esconder um índice.
        {missing.length > 0 && ` Sem dados ainda: ${missing.join(", ")} (atualize as cotações).`}
      </p>
    </>
  );
}

function Patrimony({ frame, p }: { frame: Frame; p: Palette }) {
  const option = useMemo<EChartsCoreOption>(() => {
    const gain = frame.slots.map((m) => (m?.value == null ? null : Number(m.value) - Number(m.invested)));
    return {
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => (v == null ? "—" : brl(v)) },
      legend: { ...(base(p).legend as object), data: ["Valor aplicado", "Ganho de capital"] },
      color: [p.series[0], p.series[2]],
      xAxis: { type: "category", data: frame.labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: [
        {
          name: "Valor aplicado",
          type: "bar",
          stack: "p",
          barMaxWidth: 28,
          data: frame.slots.map((m) => (m ? Number(m.invested) : null)),
          itemStyle: { borderColor: p.surface, borderWidth: 1 },
        },
        {
          name: "Ganho de capital",
          type: "bar",
          stack: "p",
          barMaxWidth: 28,
          data: gain.map((g) => ({
            value: g,
            itemStyle: { color: g != null && g < 0 ? p.neg : p.series[2], borderRadius: g != null && g < 0 ? [0, 0, 4, 4] : [4, 4, 0, 0] },
          })),
          itemStyle: { borderColor: p.surface, borderWidth: 1 },
        },
      ],
    };
  }, [frame, p]);
  return (
    <Chart
      option={option}
      ariaLabel={`Evolução do patrimônio: valor aplicado e ganho de capital por ${frame.days ? "dia" : "mês"}`}
    />
  );
}

function Profit({ frame, byYear, p }: { frame: Frame; byYear: boolean; p: Palette }) {
  const rows = useMemo(
    () =>
      byYear
        ? profitsByYear(frame.slots.filter((m) => m != null)).map((r) => ({ label: String(r.year), value: r.profit }))
        : frame.slots.map((m, i) => ({ label: frame.labels[i], value: m?.profit == null ? null : Number(m.profit) })),
    [frame, byYear],
  );
  const option = useMemo<EChartsCoreOption>(
    () => ({
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => (v == null ? "—" : brl(v)) },
      xAxis: { type: "category", data: rows.map((r) => r.label), ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: [
        {
          name: "Lucro / prejuízo",
          type: "bar",
          data: rows.map((r) => ({
            value: r.value,
            itemStyle: { color: (r.value ?? 0) < 0 ? p.neg : p.pos, borderRadius: (r.value ?? 0) < 0 ? [0, 0, 4, 4] : [4, 4, 0, 0] },
          })),
          barMaxWidth: 28,
        },
      ],
      legend: { show: false },
      grid: { left: 8, right: 16, top: 16, bottom: 8, containLabel: true },
    }),
    [rows, p],
  );
  return <Chart option={option} ariaLabel="Lucro ou prejuízo em reais por período" />;
}

function SameFlows({ frame, p }: { frame: Frame; p: Palette }) {
  const option = useMemo<EChartsCoreOption>(() => {
    const num = (v: string | null | undefined) => (v == null ? null : Number(v));
    const series = [
      { name: "Carteira", data: frame.slots.map((m) => num(m?.value)) },
      ...frame.bench.map((bm) => ({ name: `No ${bm.name}`, data: bm.same_flows.map(num) })),
    ];
    return {
      ...base(p),
      color: p.series,
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => (v == null ? "—" : brl(v)) },
      xAxis: { type: "category", data: frame.labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", scale: true, ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: series.map((s, i) => ({
        ...line,
        name: s.name,
        data: s.data,
        showSymbol: !frame.days,
        symbolSize: 8,
        lineStyle: { width: i === 0 ? 3 : 2 },
        emphasis: { focus: "series" },
      })),
    };
  }, [frame, p]);
  return (
    <>
      <Chart
        option={option}
        height={320}
        ariaLabel="Patrimônio da carteira e quanto teria se os mesmos aportes fossem para cada índice"
      />
      <p className="muted small">Cada compra e venda da carteira entra ou sai do índice na mesma data e pelo mesmo valor.</p>
    </>
  );
}

function Contributions({ frame, p, names }: { frame: Frame; p: Palette; names: Record<string, string> }) {
  const rows = frame.contributions
    .map((c) => {
      const any = c.profits.some((v) => v != null);
      // Período sem posição não tem resultado (null); com posição e sem cotação também é null.
      return { asset: assetLabel(c.asset, names), value: any ? c.profits.reduce((s, v) => s + (v == null ? 0 : Number(v)), 0) : null };
    })
    .filter((r): r is { asset: string; value: number } => r.value != null && r.value !== 0)
    .sort((x, y) => y.value - x.value);
  const option = useMemo<EChartsCoreOption>(
    () => ({
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", axisPointer: { type: "shadow" }, valueFormatter: (v: number) => brl(v) },
      yAxis: { type: "category", data: rows.map((r) => r.asset), inverse: true, ...axisStyle(p), splitLine: { show: false } },
      xAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: [
        {
          name: "Contribuição",
          type: "bar",
          barMaxWidth: 18,
          data: rows.map((r) => ({
            value: r.value,
            itemStyle: { color: r.value < 0 ? p.neg : p.pos, borderRadius: r.value < 0 ? [4, 0, 0, 4] : [0, 4, 4, 0] },
          })),
          label: { show: true, position: "right", color: p.ink, formatter: (x: { value: number }) => compact(x.value) },
        },
      ],
      legend: { show: false },
      grid: { left: 8, right: 56, top: 8, bottom: 8, containLabel: true },
    }),
    [rows, p],
  );
  if (rows.length === 0) return <p className="muted">Sem resultado no período.</p>;
  return (
    <>
      <Chart option={option} height={Math.max(160, rows.length * 32 + 40)} ariaLabel="Lucro de cada ativo no período" />
      <p className="muted small">
        Lucro de cada ativo no período, somando os {frame.days ? "dias" : "meses"} (exterior em reais, com o câmbio).
      </p>
    </>
  );
}

function Allocation({ view, p, open, setOpen }: { view: DashboardView; p: Palette; open: string | null; setOpen: (s: string | null) => void }) {
  const total = view.allocation.reduce((s, x) => s + Number(x.value ?? 0), 0);
  const option = useMemo<EChartsCoreOption>(
    () => ({
      ...base(p),
      color: p.series,
      legend: { show: false },
      tooltip: {
        ...(base(p).tooltip as object),
        trigger: "item",
        formatter: (x: { name: string; value: number; percent: number }) =>
          `${x.name}<br/>${brl(x.value)} · ${x.percent.toLocaleString("pt-BR", { maximumFractionDigits: 1 })}%`,
      },
      series: [
        {
          type: "pie",
          radius: ["55%", "80%"],
          itemStyle: { borderColor: p.surface, borderWidth: 2, borderRadius: 4 },
          label: { show: false },
          data: view.allocation.map((s) => ({ name: s.label, value: Number(s.value ?? 0) })),
        },
      ],
    }),
    [view, p],
  );
  if (view.allocation.length === 0) return <p className="muted">Nenhuma posição aberta.</p>;
  return (
    <div className="alloc">
      <Chart
        option={option}
        height={200}
        ariaLabel="Divisão da carteira por tipo de ativo"
        onSelect={(i) => setOpen(view.allocation[i].label === open ? null : view.allocation[i].label)}
      />
      <table className="alloc-table">
        <tbody>
          {view.allocation.map((s, i) => (
            <FragmentRows key={s.label}>
              <tr className="clickable" onClick={() => setOpen(open === s.label ? null : s.label)}>
                <td>
                  <span className="swatch" style={{ background: p.series[i % p.series.length] }} />
                  {open === s.label ? "▾ " : "▸ "}
                  {s.label} <span className="muted">({s.assets.length})</span>
                </td>
                <td className="num">{money(s.value)}</td>
                <td className="num">{total ? ((Number(s.value ?? 0) / total) * 100).toLocaleString("pt-BR", { maximumFractionDigits: 1 }) : "—"}%</td>
              </tr>
              {open === s.label &&
                s.assets.map((x) => (
                  <tr key={x.asset} className="sub">
                    <td>{assetLabel(x.asset, view.names)}</td>
                    <td className="num">{money(x.value)}</td>
                    <td className="num">{total ? ((Number(x.value ?? 0) / total) * 100).toLocaleString("pt-BR", { maximumFractionDigits: 1 }) : "—"}%</td>
                  </tr>
                ))}
            </FragmentRows>
          ))}
        </tbody>
      </table>
    </div>
  );
}

function FragmentRows({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}

function Drawdown({ frame, p }: { frame: Frame; p: Palette }) {
  const dd = useMemo(() => {
    let level = 1;
    let peak = 1;
    let broken = false;
    return frame.slots.map((m) => {
      if (!m) return null;
      if (m.rate == null || broken) {
        broken = true;
        return null;
      }
      level *= 1 + Number(m.rate);
      peak = Math.max(peak, level);
      return level / peak - 1;
    });
  }, [frame]);
  const worst = dd.reduce<number | null>((w, v) => (v == null ? w : w == null ? v : Math.min(w, v)), null);
  const option = useMemo<EChartsCoreOption>(
    () => ({
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => pct(v) },
      xAxis: { type: "category", data: frame.labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", max: 0, ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => pct(v) } },
      series: [
        {
          ...line,
          name: "Queda desde o pico",
          data: dd,
          showSymbol: !frame.days,
          symbolSize: 6,
          lineStyle: { width: 2, color: p.neg },
          itemStyle: { color: p.neg },
          areaStyle: { color: p.neg, opacity: 0.15 },
        },
      ],
      legend: { show: false },
      grid: { left: 8, right: 16, top: 16, bottom: 8, containLabel: true },
    }),
    [dd, frame, p],
  );
  return (
    <>
      <Chart option={option} height={220} ariaLabel="Queda da rentabilidade acumulada em relação ao maior valor" />
      <p className="muted small">Maior queda no período: {pct(worst)}.</p>
    </>
  );
}

function Realized({ view, p }: { view: DashboardView; p: Palette }) {
  const rows = view.positions
    .map((x) => ({ asset: assetLabel(x.asset, view.names), realized: Number(x.realized_brl ?? 0), unrealized: Number(x.unrealized_brl ?? 0) }))
    .filter((r) => r.realized !== 0 || r.unrealized !== 0);
  const tot = rows.reduce((s, r) => ({ realized: s.realized + r.realized, unrealized: s.unrealized + r.unrealized }), { realized: 0, unrealized: 0 });
  const option = useMemo<EChartsCoreOption>(
    () => ({
      ...base(p),
      color: [p.series[0], p.series[1]],
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", axisPointer: { type: "shadow" }, valueFormatter: (v: number) => brl(v) },
      yAxis: { type: "category", data: rows.map((r) => r.asset), inverse: true, ...axisStyle(p), splitLine: { show: false } },
      xAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: [
        { name: "Realizado (vendas)", type: "bar", barMaxWidth: 14, data: rows.map((r) => r.realized), itemStyle: { borderRadius: 3 } },
        { name: "Não realizado (no papel)", type: "bar", barMaxWidth: 14, data: rows.map((r) => r.unrealized), itemStyle: { borderRadius: 3 } },
      ],
      grid: { left: 8, right: 32, top: 40, bottom: 8, containLabel: true },
    }),
    [rows, p],
  );
  if (rows.length === 0) return <p className="muted">Sem resultado ainda.</p>;
  return (
    <>
      <Chart option={option} height={Math.max(180, rows.length * 44 + 60)} ariaLabel="Lucro realizado e não realizado por ativo" />
      <p className="muted small">
        Total realizado {brl(tot.realized)} · não realizado {brl(tot.unrealized)}.
      </p>
    </>
  );
}

function Timeline({ view }: { view: DashboardView }) {
  const start = new Date(view.timeline.reduce((m, s) => (s.start < m ? s.start : m), view.as_of)).getTime();
  const end = new Date(view.as_of).getTime();
  const span = Math.max(1, end - start);
  const pos = (iso: string) => ((new Date(iso).getTime() - start) / span) * 100;
  const days = (a: string, b: string) => Math.round((new Date(b).getTime() - new Date(a).getTime()) / 86400000);
  return (
    <div className="timeline">
      {view.timeline.map((s, i) => {
        const left = pos(s.start);
        const right = s.end ? pos(s.end) : 100;
        const d = days(s.start, s.end ?? view.as_of);
        return (
          <div className="tl-row" key={`${s.asset}-${i}`}>
            <span className="tl-label">{assetLabel(s.asset, view.names)}</span>
            <div className="tl-track">
              <div
                className={s.end ? "tl-bar closed" : "tl-bar"}
                style={{ left: `${left}%`, width: `${Math.max(0.8, right - left)}%` }}
                title={`${assetLabel(s.asset, view.names)}: ${date(s.start)} a ${s.end ? date(s.end) : "hoje"} (${d} dias) · ${s.accounts.join(", ")}`}
              />
            </div>
            <span className="tl-days muted">{d} dias</span>
          </div>
        );
      })}
      <div className="tl-axis muted small">
        <span>{date(view.timeline.reduce((m, s) => (s.start < m ? s.start : m), view.as_of))}</span>
        <span>hoje</span>
      </div>
      <p className="muted small">Tempo real em que cada ativo esteve na carteira; é o tempo usado no cálculo da rentabilidade. Barras claras: posições já vendidas.</p>
    </div>
  );
}

/** "R$ 12,3 mil" para eixos. */
function compact(v: number): string {
  const abs = Math.abs(v);
  if (abs >= 1e6) return `R$ ${(v / 1e6).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mi`;
  if (abs >= 1e3) return `R$ ${(v / 1e3).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mil`;
  return `R$ ${v.toLocaleString("pt-BR", { maximumFractionDigits: 0 })}`;
}

