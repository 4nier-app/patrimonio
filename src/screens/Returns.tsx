import { useEffect, useMemo, useState } from "react";
import { errorMessage, market, type MonthResult, type ReturnsView, type ScopeReturns, type YearResult } from "../api";
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

function Split({ data, month, year }: { data: ScopeReturns; month?: MonthResult; year?: YearResult }) {
  const n = nativeOf(data, month, year);
  if (!n) return null;
  const usdRate = n.usd?.rate ?? n.usdYear?.rate ?? null;
  return (
    <div className="small muted" title="Rentabilidade em dólar · efeito do câmbio">
      US$ {percent(usdRate)} · câmbio {percent(n.fx)}
    </div>
  );
}

export default function Returns() {
  const [view, setView] = useState<ReturnsView | null>(null);
  const [year, setYear] = useState<number | null>(null);
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
  const rows: { label: string; data: ScopeReturns }[] = view
    ? [{ label: "Carteira", data: view.portfolio }, ...view.assets.map((a) => ({ label: a.asset ?? "", data: a }))]
    : [];

  return (
    <section>
      <div className="section-head">
        <div>
          <h2>Rentabilidade</h2>
          <p className="muted">
            O capital conta a partir do dia exato da compra e deixa de contar no dia exato da venda. Cada mês usa o método
            Modified Dietz com peso diário; o ano encadeia os meses. Clique num valor para ver a memória de cálculo.
            Proventos não entram no cálculo. Tudo em reais: ativos no exterior são convertidos pela PTAX e mostram também
            o resultado em dólar e o efeito do câmbio.
          </p>
        </div>
        {years.length > 0 && (
          <label className="short">
            Ano
            <select value={year ?? ""} onChange={(e) => setYear(Number(e.target.value))}>
              {years.map((y) => (
                <option key={y} value={y}>
                  {y}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      {error && <p className="error">{error}</p>}
      {view && rows[0].data.months.length === 0 && <p className="muted">Sem lançamentos para calcular.</p>}

      {year != null && rows.length > 0 && rows[0].data.months.length > 0 && (
        <div className="scroll">
          <table className="returns">
            <thead>
              <tr>
                <th>Ativo</th>
                {Array.from({ length: 12 }, (_, i) => (
                  <th key={i} className="num">
                    {monthLabel(i + 1)}
                  </th>
                ))}
                <th className="num">{year}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map(({ label, data }) => {
                const y = data.years.find((r) => r.year === year);
                if (!y && label !== "Carteira") return null;
                return (
                  <tr key={label} className={label === "Carteira" ? "total-row" : ""}>
                    <td>
                      <strong>{label}</strong>
                    </td>
                    {Array.from({ length: 12 }, (_, i) => {
                      const m = data.months.find((r) => r.year === year && r.month === i + 1);
                      return (
                        <td
                          key={i}
                          className={`num cell ${m ? "clickable " + tone(m.rate) : ""}`}
                          onClick={() => m && setSelected({ scope: label, data, month: m })}
                        >
                          {m ? (
                            <>
                              {percent(m.rate)}
                              <div className="small muted">{money(m.profit)}</div>
                              <Split data={data} month={m} />
                            </>
                          ) : (
                            ""
                          )}
                        </td>
                      );
                    })}
                    <td
                      className={`num cell ${y ? "clickable " + tone(y.rate) : ""}`}
                      onClick={() => y && setSelected({ scope: label, data, year: y })}
                    >
                      {y && (
                        <>
                          <strong>{percent(y.rate)}</strong>
                          <div className="small muted">{money(y.profit)}</div>
                          <Split data={data} year={y} />
                        </>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {selected && (
        <div className="card memo spaced">
          <div className="section-head">
            <h3>
              Memória de cálculo · {selected.scope} ·{" "}
              {selected.month ? `${monthLabel(selected.month.month)}/${selected.month.year}` : `ano ${selected.year?.year}`}
            </h3>
            <button className="secondary" onClick={() => setSelected(null)}>
              Fechar
            </button>
          </div>
          {selected.month && (
            <>
              <p className="muted">
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
        </div>
      )}
    </section>
  );
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
