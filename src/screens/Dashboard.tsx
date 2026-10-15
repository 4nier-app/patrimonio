import { useEffect, useMemo, useState } from "react";
import type { EChartsCoreOption } from "echarts/core";
import { dashboardApi, errorMessage, type DashboardView } from "../api";
import Chart, { axisStyle, base, usePalette, type Palette } from "../charts/Chart";
import { cumulative, monthKey, monthName, monthTag, PERIODS, periodRange, profitsByYear, type Period } from "../dashboard";
import { date, money, percent } from "../format";

type Memo = { title: string; lines: string[] };

const brl = (n: number) => money(String(n));
const pct = (n: number | null) => (n == null ? "—" : percent(String(n)));

export default function Dashboard({ refresh }: { refresh: number }) {
  const [view, setView] = useState<DashboardView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [period, setPeriod] = useState<Period>("inicio");
  const [byYear, setByYear] = useState(false);
  const [inMoney, setInMoney] = useState(false);
  const [memo, setMemo] = useState<Memo | null>(null);
  const [open, setOpen] = useState<string | null>(null);
  const p = usePalette();

  useEffect(() => {
    dashboardApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }, [refresh]);

  if (error) return <p className="error">{error}</p>;
  if (!view) return <p className="muted">Calculando…</p>;
  if (view.months.length === 0)
    return (
      <section>
        <h2>Painel</h2>
        <p className="muted">Nenhum lançamento ainda. Comece pela aba Lançamentos.</p>
      </section>
    );

  const years = [...new Set(view.months.map((m) => m.year))];

  return (
    <section className="dashboard">
      <div className="dash-head">
        <div>
          <h2>Painel</h2>
          <p className="muted">
            Clique em qualquer barra, ponto ou cartão para ver a memória de cálculo. Valores em reais; exterior pela PTAX.
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

      <div className="dash-grid">
        <Block
          title="Comparação com índices"
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
          {inMoney ? (
            <SameFlows view={view} period={period} p={p} onMemo={setMemo} />
          ) : (
            <Comparison view={view} period={period} p={p} onMemo={setMemo} />
          )}
        </Block>
        <Block title="Evolução do patrimônio">
          <Patrimony view={view} period={period} p={p} onMemo={setMemo} />
        </Block>
        <Block
          title="Lucro / prejuízo em R$"
          extra={
            <div className="segmented small" role="group" aria-label="Agrupar">
              <button className={!byYear ? "seg active" : "seg"} onClick={() => setByYear(false)}>
                Mês
              </button>
              <button className={byYear ? "seg active" : "seg"} onClick={() => setByYear(true)}>
                Ano
              </button>
            </div>
          }
        >
          <Profit view={view} period={period} byYear={byYear} p={p} onMemo={setMemo} />
        </Block>
        <Block title="O que puxou o resultado">
          <Contributions view={view} period={period} p={p} />
        </Block>
        <Block title="Ativos na carteira hoje">
          <Allocation view={view} p={p} open={open} setOpen={setOpen} />
        </Block>
        <Block title="Queda desde o pico">
          <Drawdown view={view} period={period} p={p} onMemo={setMemo} />
        </Block>
        <Block title="Realizado × não realizado">
          <Realized view={view} p={p} />
        </Block>
        <Block title="Linha do tempo das posições" wide>
          <Timeline view={view} />
        </Block>
      </div>

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

function useRange(view: DashboardView, period: Period) {
  return useMemo(() => {
    const [a, b] = periodRange(view.months, period);
    return { a, b, months: view.months.slice(a, b), labels: view.months.slice(a, b).map(monthTag) };
  }, [view, period]);
}

function monthMemo(view: DashboardView, i: number, extra: string[] = []): Memo {
  const m = view.months[i];
  return {
    title: `Carteira em ${monthTag(m)}`,
    lines: [...m.explanation, ...extra, ...view.benchmarks.map((b) => b.explanations[i]?.[0]).filter(Boolean)],
  };
}

function Comparison({ view, period, p, onMemo }: { view: DashboardView; period: Period; p: Palette; onMemo: (m: Memo) => void }) {
  const { a, b, months, labels } = useRange(view, period);
  const option = useMemo<EChartsCoreOption>(() => {
    const series = [
      { name: "Carteira", data: cumulative(months.map((m) => m.rate)) },
      ...view.benchmarks.map((bm) => ({ name: bm.name, data: cumulative(bm.rates.slice(a, b)) })),
    ];
    return {
      ...base(p),
      color: p.series,
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => pct(v) },
      xAxis: { type: "category", data: labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => pct(v) } },
      series: series.map((s, i) => ({
        name: s.name,
        type: "line",
        data: s.data,
        showSymbol: true,
        symbolSize: 8,
        lineStyle: { width: i === 0 ? 3 : 2 },
        endLabel: { show: true, color: p.ink, formatter: (x: { value: number }) => pct(x.value) },
        emphasis: { focus: "series" },
      })),
      grid: { left: 8, right: 64, top: 40, bottom: 8, containLabel: true },
    };
  }, [view, a, b, labels, months, p]);
  const missing = view.benchmarks.filter((bm) => bm.rates.slice(a, b).every((r) => r == null)).map((bm) => bm.name);
  return (
    <>
      <Chart
        option={option}
        height={320}
        ariaLabel="Rentabilidade acumulada da carteira e dos índices"
        onSelect={(i) => onMemo(monthMemo(view, a + i))}
      />
      <p className="muted small">
        Acumulado no período, com os índices medidos nos mesmos dias em que havia dinheiro na carteira. Clique no nome
        para mostrar ou esconder um índice.
        {missing.length > 0 && ` Sem dados ainda: ${missing.join(", ")} (atualize as cotações).`}
      </p>
    </>
  );
}

function Patrimony({ view, period, p, onMemo }: { view: DashboardView; period: Period; p: Palette; onMemo: (m: Memo) => void }) {
  const { a, months, labels } = useRange(view, period);
  const option = useMemo<EChartsCoreOption>(() => {
    const gain = months.map((m) => (m.value == null ? null : Number(m.value) - Number(m.invested)));
    return {
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => (v == null ? "—" : brl(v)) },
      legend: { ...(base(p).legend as object), data: ["Valor aplicado", "Ganho de capital"] },
      color: [p.series[0], p.series[2]],
      xAxis: { type: "category", data: labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: [
        {
          name: "Valor aplicado",
          type: "bar",
          stack: "p",
          barMaxWidth: 28,
          data: months.map((m) => Number(m.invested)),
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
  }, [months, labels, p]);
  return (
    <Chart
      option={option}
      ariaLabel="Evolução do patrimônio: valor aplicado e ganho de capital por mês"
      onSelect={(i) => {
        const m = view.months[a + i];
        onMemo(
          monthMemo(view, a + i, [
            `Valor aplicado até ${date(m.period_end)} = compras − vendas (com taxas) = ${money(m.invested)}`,
            m.value == null
              ? "Ganho de capital indisponível (falta cotação)"
              : `Ganho de capital = patrimônio − aplicado = ${money(m.value)} − ${money(m.invested)} = ${brl(Number(m.value) - Number(m.invested))}`,
          ]),
        );
      }}
    />
  );
}

function Profit({ view, period, byYear, p, onMemo }: { view: DashboardView; period: Period; byYear: boolean; p: Palette; onMemo: (m: Memo) => void }) {
  const { a, months, labels } = useRange(view, period);
  const rows = byYear
    ? profitsByYear(months).map((r) => ({ label: String(r.year), value: r.profit }))
    : months.map((m, i) => ({ label: labels[i], value: m.profit == null ? null : Number(m.profit) }));
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
  return (
    <Chart
      option={option}
      ariaLabel="Lucro ou prejuízo em reais por período"
      onSelect={(i) => {
        if (byYear) {
          const year = Number(rows[i].label);
          onMemo({
            title: `Lucro de ${year}`,
            lines: months
              .filter((m) => m.year === year)
              .map((m) => `${monthTag(m)}: ${m.profit == null ? "indisponível" : money(m.profit)}`)
              .concat(`Total = ${rows[i].value == null ? "indisponível" : brl(rows[i].value as number)}`),
          });
        } else onMemo(monthMemo(view, a + i));
      }}
    />
  );
}

function SameFlows({ view, period, p, onMemo }: { view: DashboardView; period: Period; p: Palette; onMemo: (m: Memo) => void }) {
  const { a, b, months, labels } = useRange(view, period);
  const option = useMemo<EChartsCoreOption>(() => {
    const series = [
      { name: "Carteira", data: months.map((m) => (m.value == null ? null : Number(m.value))) },
      ...view.benchmarks.map((bm) => ({ name: `No ${bm.name}`, data: bm.same_flows.slice(a, b).map((v) => (v == null ? null : Number(v))) })),
    ];
    return {
      ...base(p),
      color: p.series,
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => (v == null ? "—" : brl(v)) },
      xAxis: { type: "category", data: labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", scale: true, ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: series.map((s, i) => ({
        name: s.name,
        type: "line",
        data: s.data,
        symbolSize: 8,
        lineStyle: { width: i === 0 ? 3 : 2 },
        emphasis: { focus: "series" },
      })),
    };
  }, [view, a, b, months, labels, p]);
  return (
    <>
      <Chart
        option={option}
        height={320}
        ariaLabel="Patrimônio da carteira e quanto teria se os mesmos aportes fossem para cada índice"
        onSelect={(i) => {
          const idx = a + i;
          onMemo({
            title: `Mesmos aportes em ${monthTag(view.months[idx])}`,
            lines: [
              `Carteira: ${money(view.months[idx].value)}`,
              ...view.benchmarks.flatMap((bm) => bm.explanations[idx] ?? []),
            ],
          });
        }}
      />
      <p className="muted small">
        Cada compra e venda da carteira entra ou sai do índice na mesma data e pelo mesmo valor. Clique num mês para ver a
        conta.
      </p>
    </>
  );
}

function Contributions({ view, period, p }: { view: DashboardView; period: Period; p: Palette }) {
  const { a, b } = useRange(view, period);
  const rows = view.contributions
    .map((c) => {
      const slice = c.profits.slice(a, b);
      const any = slice.some((v) => v != null);
      // Mês sem posição não tem resultado (null); mês com posição e sem cotação também é null.
      return { asset: c.asset, value: any ? slice.reduce((s, v) => s + (v == null ? 0 : Number(v)), 0) : null };
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
      <p className="muted small">Lucro de cada ativo no período, somando os meses (exterior em reais, com o câmbio).</p>
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
                    <td>{x.asset}</td>
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

function Drawdown({ view, period, p, onMemo }: { view: DashboardView; period: Period; p: Palette; onMemo: (m: Memo) => void }) {
  const { a, months, labels } = useRange(view, period);
  const dd = useMemo(() => {
    let level = 1;
    let peak = 1;
    let broken = false;
    return months.map((m) => {
      if (m.rate == null || broken) {
        broken = true;
        return null;
      }
      level *= 1 + Number(m.rate);
      peak = Math.max(peak, level);
      return level / peak - 1;
    });
  }, [months]);
  const worst = dd.reduce<number | null>((w, v) => (v == null ? w : w == null ? v : Math.min(w, v)), null);
  const option = useMemo<EChartsCoreOption>(
    () => ({
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => pct(v) },
      xAxis: { type: "category", data: labels, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", max: 0, ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => pct(v) } },
      series: [
        {
          name: "Queda desde o pico",
          type: "line",
          data: dd,
          symbolSize: 6,
          lineStyle: { width: 2, color: p.neg },
          itemStyle: { color: p.neg },
          areaStyle: { color: p.neg, opacity: 0.15 },
        },
      ],
      legend: { show: false },
      grid: { left: 8, right: 16, top: 16, bottom: 8, containLabel: true },
    }),
    [dd, labels, p],
  );
  return (
    <>
      <Chart
        option={option}
        height={220}
        ariaLabel="Queda da rentabilidade acumulada em relação ao maior valor"
        onSelect={(i) =>
          onMemo({
            title: `Queda desde o pico em ${labels[i]}`,
            lines: [
              "Acumulado = (1 + r₁)·(1 + r₂)·… no período escolhido; queda = acumulado ÷ maior acumulado anterior − 1",
              `Queda em ${labels[i]} = ${pct(dd[i])}`,
              ...view.months[a + i].explanation.slice(-1),
            ],
          })
        }
      />
      <p className="muted small">Maior queda no período: {pct(worst)}.</p>
    </>
  );
}

function Realized({ view, p }: { view: DashboardView; p: Palette }) {
  const rows = view.positions
    .map((x) => ({ asset: x.asset, realized: Number(x.realized_brl ?? 0), unrealized: Number(x.unrealized_brl ?? 0) }))
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
            <span className="tl-label">{s.asset}</span>
            <div className="tl-track">
              <div
                className={s.end ? "tl-bar closed" : "tl-bar"}
                style={{ left: `${left}%`, width: `${Math.max(0.8, right - left)}%` }}
                title={`${s.asset}: ${date(s.start)} a ${s.end ? date(s.end) : "hoje"} (${d} dias) · ${s.accounts.join(", ")}`}
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

