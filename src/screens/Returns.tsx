import { useEffect, useMemo, useState } from "react";
import { errorMessage, market, type MonthResult, type ReturnsView, type ScopeReturns, type YearResult } from "../api";
import { chained } from "../dashboard";
import { date, money, monthLabel, percent, tone } from "../format";

type Selected = { scope: string; data: ScopeReturns; month?: MonthResult; year?: YearResult };

/** Mesmo período em dólar e efeito do câmbio, para ativos no exterior. */
function nativeOf(data: ScopeReturns, month?: MonthResult, year?: YearResult) {
  const n = data.native;
  if (!n) return null;
  if (month) {
    const i = data.months.indexOf(month);
    return { usd: n.months.find((r) => r.year === month.year && r.month === month.month) ?? null, usdYear: null, fx: n.fx_months[i] ?? null };
  }
  if (year) {
    const i = data.years.indexOf(year);
    return { usd: null, usdYear: n.years.find((r) => r.year === year.year) ?? null, fx: n.fx_years[i] ?? null };
  }
  return null;
}

/** "US$ 1,2% · câmbio 0,8%", só quando há os dois números. */
function Split({ data, month, year }: { data: ScopeReturns; month?: MonthResult; year?: YearResult }) {
  const n = nativeOf(data, month, year);
  const usd = n?.usd?.rate ?? n?.usdYear?.rate ?? null;
  if (!n || usd == null || n.fx == null) return null;
  return (
    <span className="ret-split" title="Rentabilidade em dólar e efeito do câmbio">
      US$ {percent(usd)}
      <br />
      câmbio {percent(n.fx)}
    </span>
  );
}

/** Taxa e lucro; sem cotação mostra o aviso em vez de um traço. */
function Value({ rate, profit, big }: { rate: string | null; profit: string | null; big?: boolean }) {
  if (rate == null && profit == null) return <span className="ret-missing">sem cotação</span>;
  return (
    <>
      <span className={`ret-rate ${big ? "big" : ""} ${tone(rate)}`}>{rate == null ? "" : percent(rate)}</span>
      {profit != null && <span className="ret-profit">{money(profit)}</span>}
    </>
  );
}

/** Desde o início: encadeia todos os meses e soma os lucros. */
function sinceStart(data: ScopeReturns) {
  const rate = chained(data.months.map((m) => m.rate));
  const any = data.months.some((m) => m.profit != null);
  const profit = data.months.every((m) => m.profit != null)
    ? data.months.reduce((s, m) => s + Number(m.profit), 0)
    : null;
  return { rate: rate == null ? null : String(rate), profit: any && profit != null ? profit.toFixed(2) : null };
}

/** Situação do ativo: na carteira ou vendido (data da venda que zerou). */
function status(data: ScopeReturns): { open: boolean; label: string } {
  const last = data.months[data.months.length - 1];
  if (!last || !last.closed) return { open: true, label: "Na carteira" };
  const d = new Date(`${last.period_end}T12:00:00`);
  d.setDate(d.getDate() + 1);
  return { open: false, label: `Vendido em ${date(d.toISOString().slice(0, 10))}` };
}

function Months({
  label,
  data,
  year,
  onPick,
}: {
  label: string;
  data: ScopeReturns;
  year: number;
  onPick: (s: Selected) => void;
}) {
  return (
    <div className="ret-months">
      {Array.from({ length: 12 }, (_, i) => {
        const m = data.months.find((r) => r.year === year && r.month === i + 1);
        if (!m)
          return (
            <div key={i} className="ret-month empty">
              <span className="ret-month-name">{monthLabel(i + 1)}</span>
            </div>
          );
        return (
          <button key={i} className={`ret-month ${tone(m.rate)}`} onClick={() => onPick({ scope: label, data, month: m })}>
            <span className="ret-month-name">{monthLabel(i + 1)}</span>
            <Value rate={m.rate} profit={m.profit} />
            <Split data={data} month={m} />
          </button>
        );
      })}
    </div>
  );
}

export default function Returns() {
  const [view, setView] = useState<ReturnsView | null>(null);
  const [year, setYear] = useState<number | null>(null);
  const [openAsset, setOpenAsset] = useState<string | null>(null);
  const [selected, setSelected] = useState<Selected | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    market
      .returns()
      .then((v) => {
        setView(v);
        const years = v.portfolio.years.map((y) => y.year);
        setYear(years.length ? years[years.length - 1] : null);
      })
      .catch((e) => setError(errorMessage(e)));
  }, []);

  const years = useMemo(() => view?.portfolio.years.map((y) => y.year) ?? [], [view]);
  const groups = useMemo(() => {
    // Só ativos com rentabilidade no ano escolhido.
    const rows = (view?.assets ?? [])
      .filter((a) => a.years.some((y) => y.year === year && y.rate != null))
      .map((a) => ({ label: a.asset ?? "", data: a, status: status(a) }));
    return [
      { title: "Na carteira", rows: rows.filter((r) => r.status.open) },
      { title: "Já vendidos", rows: rows.filter((r) => !r.status.open) },
    ].filter((g) => g.rows.length > 0);
  }, [view, year]);

  if (error) return <p className="error">{error}</p>;
  if (!view) return <p className="muted">Calculando…</p>;
  if (view.portfolio.months.length === 0 || year == null)
    return (
      <section>
        <h2>Rentabilidade</h2>
        <p className="muted">Sem lançamentos para calcular.</p>
      </section>
    );

  const pYear = view.portfolio.years.find((y) => y.year === year);
  const pAll = sinceStart(view.portfolio);

  return (
    <section className="returns-page">
      <div className="section-head">
        <div>
          <h2>Rentabilidade</h2>
          <p className="muted">
            O dinheiro conta do dia da compra ao dia da venda. Valores em reais; no exterior, também em dólar e o efeito do
            câmbio. Clique num mês ou ano para ver a conta.
          </p>
        </div>
        <div className="segmented" role="group" aria-label="Ano">
          {years.map((y) => (
            <button key={y} className={y === year ? "seg active" : "seg"} onClick={() => setYear(y)}>
              {y}
            </button>
          ))}
        </div>
      </div>

      <div className="card ret-portfolio">
        <div className="ret-head">
          <h3>Carteira</h3>
          <button className="ret-total" disabled={!pYear} onClick={() => pYear && setSelected({ scope: "Carteira", data: view.portfolio, year: pYear })}>
            <span className="ret-label">Em {year}</span>
            {pYear ? <Value rate={pYear.rate} profit={pYear.profit} big /> : <span className="ret-missing">sem carteira</span>}
          </button>
          <div className="ret-total static">
            <span className="ret-label">Desde o início</span>
            <Value rate={pAll.rate} profit={pAll.profit} big />
          </div>
        </div>
        <Months label="Carteira" data={view.portfolio} year={year} onPick={setSelected} />
      </div>

      {groups.map((g) => (
        <div key={g.title} className="card ret-assets">
          <h3>{g.title}</h3>
          <table className="ret-table">
            <thead>
              <tr>
                <th>Ativo</th>
                <th className="num">Em {year}</th>
                <th className="num">Desde o início</th>
              </tr>
            </thead>
            <tbody>
              {g.rows.map(({ label, data, status }) => {
                const y = data.years.find((r) => r.year === year);
                const all = sinceStart(data);
                const isOpen = openAsset === label;
                return (
                  <FragmentRows key={label}>
                    <tr className="clickable" onClick={() => setOpenAsset(isOpen ? null : label)}>
                      <td>
                        <span className="ret-caret">{isOpen ? "▾" : "▸"}</span>
                        <strong>{label}</strong>
                        {data.currency !== "BRL" && <span className="ret-badge">US$</span>}
                        {!status.open && <span className="ret-status">{status.label}</span>}
                      </td>
                      <td className="num">
                        {y && (
                          <>
                            <Value rate={y.rate} profit={y.profit} />
                            <Split data={data} year={y} />
                          </>
                        )}
                      </td>
                      <td className="num">
                        <Value rate={all.rate} profit={all.profit} />
                      </td>
                    </tr>
                    {isOpen && (
                      <tr className="ret-detail">
                        <td colSpan={3}>
                          <Months label={label} data={data} year={year} onPick={setSelected} />
                          {y && (
                            <button className="link" onClick={() => setSelected({ scope: label, data, year: y })}>
                              Ver a conta de {year}
                            </button>
                          )}
                        </td>
                      </tr>
                    )}
                  </FragmentRows>
                );
              })}
            </tbody>
          </table>
        </div>
      ))}

      {selected && (
        <aside className="memo-panel" role="dialog" aria-label="Memória de cálculo">
          <div className="memo-head">
            <h3>
              {selected.scope} ·{" "}
              {selected.month ? `${monthLabel(selected.month.month)}/${selected.month.year}` : `ano ${selected.year?.year}`}
            </h3>
            <button className="link" onClick={() => setSelected(null)} aria-label="Fechar">
              ✕
            </button>
          </div>
          {selected.month && (
            <>
              <p className="muted small">
                Período: {date(selected.month.period_start)} a {date(selected.month.period_end)} ({selected.month.days} dias)
                {selected.month.closed && " · posição encerrada no mês"} · fórmula {selected.month.formula_id} v
                {selected.month.formula_version}
              </p>
              <ol className="memo-lines">
                {selected.month.explanation.map((l, i) => (
                  <li key={i}>{l}</li>
                ))}
              </ol>
              <FxMemo selected={selected} />
              {selected.month.rate == null && (
                <p className="warning">
                  Falta cotação ou PTAX para algum ativo nesta data. Use “↻ Atualizar cotações” no topo.
                </p>
              )}
            </>
          )}
          {selected.year && (
            <>
              <p>Rentabilidade do ano = produto de (1 + rentabilidade de cada mês) − 1</p>
              <p className="mono">{selected.year.explanation}</p>
              <p>Lucro do ano = soma dos lucros mensais = {money(selected.year.profit)}</p>
              <FxMemo selected={selected} />
            </>
          )}
        </aside>
      )}
    </section>
  );
}

function FragmentRows({ children }: { children: React.ReactNode }) {
  return <>{children}</>;
}

/** Decomposição em ativo (dólar) e câmbio: (1 + em reais) = (1 + em dólar) × (1 + câmbio). */
function FxMemo({ selected }: { selected: Selected }) {
  const n = nativeOf(selected.data, selected.month, selected.year);
  if (!n) return null;
  const brl = selected.month?.rate ?? selected.year?.rate ?? null;
  const usd = n.usd?.rate ?? n.usdYear?.rate ?? null;
  return (
    <div className="spaced">
      <h4>Em dólar e efeito do câmbio</h4>
      <p className="mono">
        (1 + {percent(brl)} em reais) = (1 + {percent(usd)} em dólar) × (1 + {percent(n.fx)} de câmbio)
      </p>
      {n.usd && (
        <details>
          <summary>Memória de cálculo em dólar</summary>
          <ol className="memo-lines">
            {n.usd.explanation.map((l, i) => (
              <li key={i}>{l}</li>
            ))}
          </ol>
        </details>
      )}
      {n.usdYear && <p className="mono">{n.usdYear.explanation}</p>}
    </div>
  );
}
