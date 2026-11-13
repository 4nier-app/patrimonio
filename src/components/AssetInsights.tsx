import { useEffect, useState } from "react";
import { assetApi, errorMessage, type Insights, type InsightFigure } from "../api";
import { cashMask, cashTyping, date, money, percent, price, toCashMask, tone } from "../format";

type Currency = "BRL" | "USD";

/** Valor em reais digitado na simulação, como número. */
function amountOf(text: string): number {
  const n = Number(text.replace(/[^\d,]/g, "").replace(",", "."));
  return Number.isFinite(n) ? n : 0;
}

function Rate({ f, onPick, active }: { f: InsightFigure | null; onPick: (f: InsightFigure) => void; active: boolean }) {
  if (!f) return <span className="muted">—</span>;
  return (
    <button className={`link-cell ${tone(f.rate)}${active ? " active" : ""}`} onClick={() => onPick(f)} title="Ver a conta">
      {percent(f.rate)}
    </button>
  );
}

/**
 * Informações do ativo no Home broker: resumo, rentabilidade por período,
 * "se você tivesse investido", proventos pagos e "Sobre". Buscadas na hora, nas
 * fontes gratuitas; cada parte some sozinha quando a fonte não responde.
 */
export default function AssetInsights({ ticker, name, onOpen }: { ticker: string; name: string; onOpen?: (ticker: string) => void }) {
  const [data, setData] = useState<Insights | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [picked, setPicked] = useState<InsightFigure | null>(null);
  const [years, setYears] = useState(5);
  const [amount, setAmount] = useState(toCashMask("1000.00", 2).text);
  const [allDivs, setAllDivs] = useState(false);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    setData(null);
    setPicked(null);
    assetApi
      .insights(ticker)
      .then((d) => alive && setData(d))
      .catch((e) => alive && setError(errorMessage(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [ticker]);

  if (loading) return <p className="muted small spaced">Buscando rentabilidade, proventos e informações do ativo…</p>;
  if (error) return <p className="muted small spaced">Não deu para buscar as informações do ativo agora: {error}</p>;
  if (!data || (!data.price && !data.about.text && !data.about.holdings.length)) {
    return data?.warnings.length ? (
      <p className="muted small spaced">Sem acesso às fontes agora; as informações do ativo aparecem quando houver internet.</p>
    ) : null;
  }

  const cur: Currency = data.currency === "USD" ? "USD" : "BRL";
  const pick = (f: InsightFigure) => setPicked((p) => (p === f ? null : f));
  const year = data.returns[0]?.cells[2] ?? null;
  const yearBrl = cur === "USD" ? data.returns[1]?.cells[2] ?? null : null;
  const sim = data.simulation.find((s) => s.years === years);
  const value = amountOf(amount);
  const simItems = (sim?.items ?? []).filter((i) => i.figure);
  const maxSim = Math.max(1, ...simItems.map((i) => 1 + Number(i.figure!.rate)));
  const divs = allDivs ? data.dividends : data.dividends.slice(0, 8);
  const a = data.about;
  const hasAbout = a.text || a.listing || a.sector || a.manager || a.expense_ratio || a.holdings.length || a.sectors.length;

  return (
    <div className="insights">
      {data.price && (
        <div className="cards">
          <div className="card stat static">
            <span className="muted">Cotação</span>
            <strong>{price(data.price, cur)}</strong>
            {data.price_date && <span className="muted small">em {date(data.price_date)}</span>}
          </div>
          <div className="card stat static">
            <span className="muted">Variação em 12 meses</span>
            <strong className={tone(year?.rate)}>{year ? percent(year.rate) : "—"}</strong>
            {yearBrl && <span className="muted small">em reais: {percent(yearBrl.rate)}</span>}
            {!yearBrl && <span className="muted small">com os proventos reinvestidos</span>}
          </div>
          {data.dividends_12m && (
            <div className="card stat static">
              <span className="muted">Proventos em 12 meses</span>
              <strong>{price(data.dividends_12m, cur)}</strong>
              {data.yield_12m && <span className="muted small">{percent(data.yield_12m)} da cotação (dividend yield)</span>}
            </div>
          )}
          {data.dividends[0] && (
            <div className="card stat static">
              <span className="muted">Último provento</span>
              <strong>{price(data.dividends[0].value, cur)}</strong>
              <span className="muted small">data ex {date(data.dividends[0].date)}</span>
            </div>
          )}
        </div>
      )}

      {data.returns.length > 0 && (
        <div className="card">
          <h3>Rentabilidade</h3>
          <div className="scroll">
            <table className="insights-table">
              <thead>
                <tr>
                  <th></th>
                  {data.periods.map((p) => (
                    <th key={p} className="num">
                      {p}
                    </th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {data.returns.map((r) => (
                  <tr key={r.label}>
                    <td>{r.label}</td>
                    {r.cells.map((c, i) => (
                      <td key={i} className="num">
                        <Rate f={c} onPick={pick} active={picked === c} />
                      </td>
                    ))}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {picked && data.returns.some((r) => r.cells.includes(picked)) && <p className="mono explain">{picked.explanation}</p>}
          <p className="muted small">
            Com os proventos reinvestidos e antes do imposto. Clique num número para ver a conta. "—" quando o ativo ainda não existia ou falta
            dado. Fontes: Yahoo (cotação ajustada e dólar comercial) e Banco Central (IPCA).
          </p>
        </div>
      )}

      {sim && simItems.length > 0 && (
        <div className="card">
          <h3>Se você tivesse investido</h3>
          <div className="sim-controls">
            <label htmlFor="sim-valor">Valor</label>
            <input
              id="sim-valor"
              inputMode="numeric"
              value={amount}
              onChange={(e) => setAmount(cashTyping(e.target.value, amount, 2))}
              onBlur={() => setAmount(cashMask(amount, 2))}
              size={12}
            />
            <span>há</span>
            <div className="segmented small" role="group" aria-label="Há quanto tempo">
              {data.simulation.map((s) => (
                <button key={s.years} className={`seg${s.years === years ? " active" : ""}`} onClick={() => setYears(s.years)}>
                  {s.years === 1 ? "1 ano" : `${s.years} anos`}
                </button>
              ))}
            </div>
            <span className="muted small">(desde {date(sim.start)})</span>
          </div>
          <div className="sim-bars">
            {sim.items.map((i) => {
              const label = i.asset ? ticker : i.name;
              if (!i.figure) {
                return (
                  <div key={label} className="sim-bar muted">
                    <span>{label}</span>
                    <span className="small">{i.asset ? `${ticker} ainda não existia` : "sem dados do período"}</span>
                    <span />
                  </div>
                );
              }
              const factor = 1 + Number(i.figure.rate);
              return (
                <button
                  key={label}
                  className={`sim-bar${i.asset ? " me" : ""}${picked === i.figure ? " active" : ""}`}
                  onClick={() => pick(i.figure!)}
                  title="Ver a conta"
                >
                  <span>{i.asset ? <strong>{label}</strong> : label}</span>
                  <span className="track">
                    <span className="fill" style={{ width: `${Math.max(2, (factor / maxSim) * 100)}%` }} />
                  </span>
                  <span className="num">
                    {money((value * factor).toFixed(2))} <small className={tone(i.figure.rate)}>{percent(i.figure.rate)}</small>
                  </span>
                </button>
              );
            })}
          </div>
          {picked && sim.items.some((i) => i.figure === picked) && <p className="mono explain">{picked.explanation}</p>}
          <p className="muted small">
            Em reais, com os proventos reinvestidos e antes do imposto e das taxas. O S&P 500 é medido pelo IVVB11, como na Rentabilidade.
            Fontes: Yahoo (cotações, Ibovespa, IVVB11 e dólar) e Banco Central (CDI e IPCA).
          </p>
        </div>
      )}

      {data.price && (
        <div className="card">
          <h3>Proventos pagos</h3>
          {data.dividends.length === 0 ? (
            <p className="muted small">Nenhum provento pago nos últimos 5 anos{a.text?.includes("reinveste") ? ": este ETF reinveste tudo no próprio fundo." : "."}</p>
          ) : (
            <>
              <div className="scroll">
                <table className="insights-table">
                  <thead>
                    <tr>
                      <th>Data ex</th>
                      <th className="num">Valor por cota</th>
                    </tr>
                  </thead>
                  <tbody>
                    {divs.map((d) => (
                      <tr key={d.date}>
                        <td>{date(d.date)}</td>
                        <td className="num">{price(d.value, cur)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {data.dividends.length > 8 && (
                <button className="link" onClick={() => setAllDivs((v) => !v)}>
                  {allDivs ? "Ver menos" : `Ver todos (${data.dividends.length})`}
                </button>
              )}
              <p className="muted small">
                Últimos 5 anos, pela data ex, valor bruto por cota (Yahoo).
                {cur === "USD" && !ticker.endsWith(".L") && " Nos ativos dos EUA, chega 30% menos: é o imposto retido lá."}
                {cur === "BRL" && " Na B3, o Yahoo junta dividendos e JCP (o JCP tem 15% retido na fonte)."}
              </p>
            </>
          )}
        </div>
      )}

      {hasAbout && (
        <div className="card">
          <h3>Sobre {name && name !== ticker ? name : ticker}</h3>
          {a.text && <p className="about-text">{a.text}</p>}
          {a.text_source && (
            <p className="muted small">
              {a.text_fixed && a.text_date
                ? `${a.text_source}, escrito em ${date(a.text_date)}. Este texto não é atualizado pela internet: o fundo pode ter mudado depois dessa data.`
                : `Fonte do texto: ${a.text_source}${a.text_date ? `, buscado em ${date(a.text_date)}` : ""}.`}
            </p>
          )}
          {a.listing && (
            <>
              <h4>Dados oficiais da Bolsa de Londres</h4>
              <dl className="about-facts">
                <dt>Tipo</dt>
                <dd>{a.listing.kind}</dd>
                <dt>Emissor</dt>
                <dd>{a.listing.issuer}</dd>
                <dt>Nome na bolsa</dt>
                <dd>{a.listing.name}</dd>
                <dt>País</dt>
                <dd>{a.listing.country}</dd>
                <dt>ISIN</dt>
                <dd>{a.listing.isin}</dd>
                {a.listing.start && (
                  <>
                    <dt>Negociado desde</dt>
                    <dd>{date(a.listing.start)}</dd>
                  </>
                )}
                {a.listing.income && (
                  <>
                    <dt>Proventos</dt>
                    <dd>{a.listing.income}</dd>
                  </>
                )}
              </dl>
              <p className="muted small">
                Lista da Bolsa de Londres{a.listing.list_date ? ` de ${date(a.listing.list_date)}` : ""}. O app baixa a lista nova
                uma vez por mês; o que mudou depois dessa data ainda não aparece aqui.
              </p>
            </>
          )}
          {(a.sector || a.manager || a.expense_ratio || a.website) && (
            <dl className="about-facts">
              {a.sector && (
                <>
                  <dt>Setor</dt>
                  <dd>{a.sector}</dd>
                </>
              )}
              {a.manager && (
                <>
                  <dt>Gestora</dt>
                  <dd>{a.manager}</dd>
                </>
              )}
              {a.expense_ratio && (
                <>
                  <dt>Taxa de administração</dt>
                  <dd>{percent(a.expense_ratio)} ao ano</dd>
                </>
              )}
              {a.website && (
                <>
                  <dt>Site</dt>
                  <dd>{a.website}</dd>
                </>
              )}
            </dl>
          )}
          {a.holdings.length > 0 && (
            <>
              <h4>Composição: as {a.holdings.length} maiores posições</h4>
              <div className="scroll">
                <table className="insights-table">
                  <thead>
                    <tr>
                      <th>Empresa</th>
                      <th>Código</th>
                      <th className="num">% no ETF</th>
                    </tr>
                  </thead>
                  <tbody>
                    {a.holdings.map((h) => (
                      <tr key={h.symbol + h.name}>
                        <td>{h.name}</td>
                        <td>
                          {onOpen && /^[A-Z][A-Z.-]{0,9}$/.test(h.symbol) ? (
                            <button className="link" onClick={() => onOpen(h.symbol)} title="Abrir o gráfico">
                              {h.symbol}
                            </button>
                          ) : (
                            h.symbol
                          )}
                        </td>
                        <td className="num">{percent(h.weight)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
          {a.sectors.length > 0 && (
            <>
              <h4>Peso de cada setor</h4>
              <div className="sector-bars">
                {a.sectors.map((s) => (
                  <div key={s.name} className="sector-bar">
                    <span>{s.name}</span>
                    <span className="track">
                      <span className="fill" style={{ width: `${Math.max(1, Number(s.weight) * 100)}%` }} />
                    </span>
                    <span className="num">{percent(s.weight)}</span>
                  </div>
                ))}
              </div>
            </>
          )}
          {a.data_source && (
            <p className="muted small">
              Composição, setores e taxa: {a.data_source}{a.data_date ? `, buscados em ${date(a.data_date)}` : ""}. O Yahoo informa só as
              maiores posições, não a lista completa.
            </p>
          )}
        </div>
      )}
    </div>
  );
}
