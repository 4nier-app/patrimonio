import { useEffect, useState } from "react";
import { errorMessage, market, type PortfolioView } from "../api";
import { isFund } from "../currency";
import { averagePrice, date, formatDateTime, money, percent, price, quantity, tone } from "../format";

/** Valor em reais embaixo do valor em dólar. */
function Brl({ v }: { v: string | null }) {
  return <div className="muted small">{v == null ? "R$ —" : money(v)}</div>;
}

/** Código e nome do ativo; fundo aparece pelo nome, com o CNPJ embaixo. */
export function AssetName({ asset, name }: { asset: string; name?: string }) {
  if (isFund(asset) && name) {
    return (
      <>
        <strong>{name}</strong>
        <div className="muted small">CNPJ {asset}</div>
      </>
    );
  }
  return (
    <>
      <strong>{asset}</strong>
      {name && <div className="muted small">{name}</div>}
    </>
  );
}

function ratio(a: string | null, b: string | null): string | null {
  if (a == null || b == null || Number(b) === 0) return null;
  return String(Number(a) / Number(b));
}

export default function Portfolio({ refresh, onOpen }: { refresh: number; onOpen: (asset: string) => void }) {
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
            Posição de hoje pelo preço médio (compras somam valor e taxas). Clique num ativo para ver o gráfico. Cotações ficam guardadas no cofre: sem
            internet, o app usa a última que tiver.
            {view?.last_update && <> Última atualização: {formatDateTime(view.last_update)}.</>}
            {view?.fx && (
              <>
                {" "}
                Ativos no exterior: custo pela PTAX de cada compra e valor atual pela PTAX de {date(view.fx.date)} (
                {price(view.fx.rate)} por dólar).
              </>
            )}
          </p>
        </div>
      </div>

      {error && <p className="error">{error}</p>}

      {view && (
        <div className="cards">
          <div className="card stat">
            <span className="muted">Custo da posição (R$)</span>
            <strong>{money(view.cost)}</strong>
          </div>
          <div className="card stat">
            <span className="muted">Valor de mercado</span>
            <strong>{money(view.market_value)}</strong>
            {view.market_value == null && open.length > 0 && (
              <span className="muted small">falta cotação ou PTAX de algum ativo; use ↻ Atualizar cotações</span>
            )}
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
                <tr key={p.asset} className="clickable" onClick={() => onOpen(p.asset)} title="Abrir o gráfico e os detalhes">
                  <td className="asset-link">
                    <AssetName asset={p.asset} name={view?.names[p.asset]} />
                    {p.oversold && <div className="error small">Venda maior que a posição: confira os lançamentos.</div>}
                  </td>
                  <td className="num">{quantity(p.quantity)}</td>
                  <td className="num" title={`Valor exato: ${p.average_price}`}>{averagePrice(p.average_price, p.currency)}</td>
                  <td className="num">
                    {money(p.cost, p.currency)}
                    {p.currency === "USD" && <Brl v={p.cost_brl} />}
                  </td>
                  <td className="num">
                    {p.quote ? (
                      <>
                        {price(p.quote.close, p.currency)}
                        <div className="muted small">
                          {date(p.quote.date)} · {p.quote.source}
                        </div>
                      </>
                    ) : (
                      <span className="muted">sem cotação</span>
                    )}
                  </td>
                  <td className="num">
                    {money(p.market_value, p.currency)}
                    {p.currency === "USD" && <Brl v={p.market_value_brl} />}
                  </td>
                  <td className={`num ${tone(p.unrealized)}`}>
                    {money(p.unrealized, p.currency)}
                    {p.unrealized != null && <div className="small">{percent(ratio(p.unrealized, p.cost))}</div>}
                    {p.currency === "USD" && (
                      <div className={`small ${tone(p.unrealized_brl)}`}>
                        {money(p.unrealized_brl)} {percent(ratio(p.unrealized_brl, p.cost_brl))}
                      </div>
                    )}
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
                  <tr key={p.asset} className="clickable" onClick={() => onOpen(p.asset)} title="Abrir o gráfico e os detalhes">
                    <td className="asset-link">
                      <AssetName asset={p.asset} name={view?.names[p.asset]} />
                    </td>
                    <td className={`num ${tone(p.realized)}`}>
                      {money(p.realized, p.currency)}
                      {p.currency === "USD" && <Brl v={p.realized_brl} />}
                    </td>
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
