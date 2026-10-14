import { useEffect, useState } from "react";
import { errorMessage, market, type PortfolioView } from "../api";
import { date, formatDateTime, money, percent, price, quantity, tone } from "../format";

function ratio(a: string | null, b: string): string | null {
  if (a == null || Number(b) === 0) return null;
  return String(Number(a) / Number(b));
}

export default function Portfolio({ refresh }: { refresh: number }) {
  const [view, setView] = useState<PortfolioView | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    market.portfolio().then(setView).catch((e) => setError(errorMessage(e)));
  }, [refresh]);

  const open = view?.positions.filter((p) => Number(p.quantity) !== 0) ?? [];
  const closed = view?.positions.filter((p) => Number(p.quantity) === 0) ?? [];

  return (
    <section>
      <div className="section-head">
        <div>
          <h2>Carteira</h2>
          <p className="muted">
            Posição de hoje pelo preço médio (compras somam valor e taxas). Cotações ficam guardadas no cofre: sem
            internet, o app usa a última que tiver.
            {view?.last_update && <> Última atualização: {formatDateTime(view.last_update)}.</>}
          </p>
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      {view && (
        <div className="cards">
          <div className="card stat">
            <span className="muted">Custo da posição</span>
            <strong>{money(view.cost)}</strong>
          </div>
          <div className="card stat">
            <span className="muted">Valor de mercado</span>
            <strong>{money(view.market_value)}</strong>
            {view.market_value == null && open.length > 0 && <span className="muted small">falta cotação de algum ativo</span>}
          </div>
          <div className="card stat">
            <span className="muted">Resultado em aberto</span>
            <strong className={tone(view.unrealized)}>
              {money(view.unrealized)} {view.unrealized != null && <small>{percent(ratio(view.unrealized, view.cost))}</small>}
            </strong>
          </div>
          <div className="card stat">
            <span className="muted">Resultado realizado (vendas)</span>
            <strong className={tone(view.realized)}>{money(view.realized)}</strong>
          </div>
        </div>
      )}

      {view && open.length === 0 && closed.length === 0 && (
        <p className="muted">Nenhum lançamento ainda. Comece pela aba Lançamentos.</p>
      )}

      {open.length > 0 && (
        <div className="scroll">
          <table>
            <thead>
              <tr>
                <th>Ativo</th>
                <th className="num">Quantidade</th>
                <th className="num">Preço médio</th>
                <th className="num">Custo</th>
                <th className="num">Cotação</th>
                <th className="num">Valor atual</th>
                <th className="num">Resultado</th>
              </tr>
            </thead>
            <tbody>
              {open.map((p) => (
                <tr key={p.asset}>
                  <td>
                    <strong>{p.asset}</strong>
                    {p.oversold && <div className="error small">Venda maior que a posição: confira os lançamentos.</div>}
                  </td>
                  <td className="num">{quantity(p.quantity)}</td>
                  <td className="num">{price(p.average_price)}</td>
                  <td className="num">{money(p.cost)}</td>
                  <td className="num">
                    {p.quote ? (
                      <>
                        {price(p.quote.close)}
                        <div className="muted small">
                          {date(p.quote.date)} · {p.quote.source}
                        </div>
                      </>
                    ) : (
                      <span className="muted">sem cotação</span>
                    )}
                  </td>
                  <td className="num">{money(p.market_value)}</td>
                  <td className={`num ${tone(p.unrealized)}`}>
                    {money(p.unrealized)}
                    {p.unrealized != null && <div className="small">{percent(ratio(p.unrealized, p.cost))}</div>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {closed.length > 0 && (
        <>
          <h3 className="spaced">Posições encerradas</h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Ativo</th>
                  <th className="num">Resultado realizado</th>
                </tr>
              </thead>
              <tbody>
                {closed.map((p) => (
                  <tr key={p.asset}>
                    <td>{p.asset}</td>
                    <td className={`num ${tone(p.realized)}`}>{money(p.realized)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}
