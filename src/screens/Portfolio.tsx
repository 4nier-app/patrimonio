import { Fragment, useEffect, useMemo, useState } from "react";
import Intro from "../components/Intro";
import { errorMessage, isResgate, market, resgateHow, type DollarAccount, type PortfolioView, type Position } from "../api";
import ExitCostBox, { pctText } from "../components/ExitCostBox";
import { isFund, isSavings, byName } from "../currency";
import { useSort } from "../components/useSort";
import { averagePrice, date, formatDateTime, money, percent, price, quantity, tone } from "../format";

/** Valor em reais embaixo do valor em dólar. */
function Brl({ v }: { v: string | null }) {
  return <div className="muted small">{v == null ? "R$ —" : money(v)}</div>;
}

/** Conta do valor de resgate de um ativo em dólar, para o ⓘ. */
export function resgateNote(p: Pick<Position, "market_value" | "market_value_brl" | "value_fx" | "fx">): string | null {
  if (!isResgate(p.value_fx) || p.market_value == null || p.market_value_brl == null) return null;
  let t = `${money(p.market_value, "USD")} × ${resgateHow(p.value_fx!)} = ${money(p.market_value_brl)} que chegariam no Brasil`;
  if (p.fx) t += `\nPara o imposto de renda: × PTAX ${price(p.fx.rate)} (${date(p.fx.date)}) = ${money(String(Number(p.market_value) * Number(p.fx.rate)))}`;
  return t;
}

/** Valor em reais pelo valor de resgate, com a conta no ⓘ. */
function BrlValue({ p }: { p: Position }) {
  const note = resgateNote(p);
  if (!note) return <Brl v={p.market_value_brl} />;
  return (
    <div className="muted small" title={note}>
      {money(p.market_value_brl)} <span className="info-mark">ⓘ</span>
      <div>valor de resgate</div>
    </div>
  );
}

/** Corretoras em dólar: saldo, câmbio, taxas e quanto chegaria no Brasil. */
function DollarAccounts({ rows, onSaved }: { rows: DollarAccount[]; onSaved: () => void }) {
  const updated = rows.find((r) => r.commercial_updated)?.commercial_updated;
  return (
    <div className="card spaced">
      <h3 style={{ margin: "0 0 4px" }}>Corretoras em dólar: quanto chegaria no Brasil</h3>
      {updated && <p className="muted small" style={{ margin: "0 0 12px" }}>Dólar comercial atualizado em {formatDateTime(updated)}.</p>}
      <div className="scroll">
        <table>
          <thead>
            <tr>
              <th>Corretora</th>
              <th className="num">Saldo</th>
              <th className="num">Dólar usado</th>
              <th className="num">Taxas</th>
              <th className="num">Valor em reais</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const resgate = isResgate(r.value_fx);
              return (
                <Fragment key={r.account}>
                  <tr>
                    <td>
                      <strong>{r.account}</strong>
                      <div className="muted small">{resgate ? "valor de resgate" : "pela PTAX"}</div>
                    </td>
                    <td className="num">{money(r.usd, "USD")}</td>
                    <td className="num">
                      {resgate && r.commercial ? (
                        <>
                          × {price(r.commercial.rate)}
                          <div className="muted small">dólar comercial de {date(r.commercial.date)}</div>
                        </>
                      ) : r.value_fx ? (
                        <>
                          × {price(r.value_fx.rate)}
                          <div className="muted small">{resgate ? "PTAX (sem dólar comercial)" : `PTAX de ${date(r.value_fx.date)}`}</div>
                        </>
                      ) : (
                        <span className="muted">sem câmbio</span>
                      )}
                    </td>
                    <td className="num">
                      {r.fee_pct != null && r.total_pct != null ? (
                        <>
                          − {pctText(r.total_pct)}
                          <div className="muted small">
                            conversão {pctText(r.fee_pct)} + IOF {pctText(r.iof_pct)}
                          </div>
                        </>
                      ) : (
                        <span className="muted small">sem custo informado</span>
                      )}
                      <ExitCostBox account={r.account} current={r.fee_pct ?? undefined} onSaved={onSaved} compact />
                    </td>
                    <td className="num">
                      <strong>{money(r.value_brl)}</strong>
                      {resgate && <div className="muted small">chegaria no Brasil</div>}
                    </td>
                  </tr>
                  {resgate && r.ptax && (
                    <tr>
                      <td colSpan={5} className="muted small" style={{ paddingTop: 0 }}>
                        Para o imposto de renda: {money(r.usd, "USD")} × PTAX {price(r.ptax.rate)} ({date(r.ptax.date)}) = {money(r.ptax_brl)}
                      </td>
                    </tr>
                  )}
                </Fragment>
              );
            })}
          </tbody>
        </table>
      </div>
      <p className="muted small" style={{ margin: "12px 0 0" }}>
        Valor de resgate: o que chegaria na sua conta no Brasil, já descontadas a taxa de conversão da corretora e o IOF de 0,38%. É o valor usado no
        Painel, na Evolução, na Carteira e na Rentabilidade. Nos impostos vale a PTAX, como manda a lei.
      </p>
    </div>
  );
}

/** Código e nome do ativo; fundo aparece pelo nome, com o CNPJ embaixo. */
export function AssetName({ asset, name }: { asset: string; name?: string }) {
  if (isSavings(asset)) {
    return (
      <>
        <strong>{name ?? "Cofrinho"}</strong>
        <div className="muted small">Cofrinho</div>
      </>
    );
  }
  if (byName(asset) && name) {
    return (
      <>
        <strong>{name}</strong>
        <div className="muted small">{isFund(asset) ? `CNPJ ${asset}` : "Tesouro Direto"}</div>
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

  const [bump, setBump] = useState(0);

  useEffect(() => {
    market.portfolio().then(setView).catch((e) => setError(errorMessage(e)));
  }, [refresh, bump]);

  /** Depois de informar o custo: busca o dólar comercial e recalcula. */
  function exitCostSaved() {
    setBump((n) => n + 1);
    market
      .updateQuotes()
      .catch(() => {})
      .finally(() => setBump((n) => n + 1));
  }

  const allOpen = useMemo(() => view?.positions.filter((p) => Number(p.quantity) !== 0) ?? [], [view]);
  const allClosed = useMemo(() => view?.positions.filter((p) => Number(p.quantity) === 0) ?? [], [view]);
  const num = (v: string | null | undefined) => (v == null ? null : Number(v));
  const label = (a: string) => (view?.names[a] && (isSavings(a) || byName(a)) ? view.names[a] : a);
  const openSort = useSort(
    allOpen,
    {
      ativo: (p) => label(p.asset),
      custo: (p) => num(p.cost_brl),
      valor: (p) => num(p.market_value_brl),
      resultado: (p) => num(p.unrealized_brl),
      pct: (p) => (p.unrealized_brl != null && Number(p.cost_brl) ? Number(p.unrealized_brl) / Number(p.cost_brl) : null),
    },
    "carteira",
  );
  const closedSort = useSort(allClosed, { ativo: (p) => label(p.asset), realizado: (p) => num(p.realized_brl) }, "encerradas");
  const open = openSort.sorted;
  const closed = closedSort.sorted;

  return (
    <section>
      <div className="section-head">
        <div>
          <h2>Carteira</h2>
          <Intro
            more={
              <>
                Posição pelo preço médio: as compras somam valor e taxas. As cotações ficam guardadas no cofre; sem internet, o app usa a última que
                tiver.
                {view?.fx && (
                  <>
                    {" "}
                    Ativos no exterior: custo pela PTAX de cada compra e valor atual pela PTAX de {date(view.fx.date)} ({price(view.fx.rate)} por
                    dólar). Nas corretoras em dólar com o custo para trazer ao Brasil informado, o valor atual é o valor de resgate: dólar comercial
                    menos a taxa de conversão da corretora e o IOF, o que chegaria na sua conta no Brasil. Os impostos sempre usam a PTAX.
                  </>
                )}
              </>
            }
          >
            Sua posição hoje. Clique num ativo para ver o gráfico e os detalhes.
            {view?.last_update && <> Cotações de {formatDateTime(view.last_update)}.</>}
          </Intro>
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
                {openSort.header("ativo", "Ativo")}
                <th className="num">Quantidade</th>
                <th className="num">Preço médio</th>
                {openSort.header("custo", "Custo", { numeric: true })}
                <th className="num">Cotação</th>
                {openSort.header("valor", "Valor atual", { numeric: true })}
                {openSort.header("resultado", "Resultado", { numeric: true, title: "Clique de novo para inverter; ordena pelo valor em reais" })}
              </tr>
            </thead>
            <tbody>
              {open.map((p) =>
                isSavings(p.asset) ? (
                  <tr key={p.asset} title="Movimentos e saldos em Evolução › Contas e reservas">
                    <td>
                      <AssetName asset={p.asset} name={view?.names[p.asset]} />
                    </td>
                    <td className="num muted">—</td>
                    <td className="num muted">—</td>
                    <td className="num">{money(p.cost)}</td>
                    <td className="num muted small">pelo último saldo informado</td>
                    <td className="num">{money(p.market_value)}</td>
                    <td className={`num ${tone(p.unrealized)}`}>
                      {money(p.unrealized)}
                      {p.unrealized != null && <div className="small">{percent(ratio(p.unrealized, p.cost))}</div>}
                    </td>
                  </tr>
                ) : (
                <tr key={p.asset} className="clickable go"
                  tabIndex={0}
                  onClick={() => onOpen(p.asset)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") onOpen(p.asset);
                  }}
                  title="Abrir o gráfico e os detalhes"
                >
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
                    {p.currency === "USD" && <BrlValue p={p} />}
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
                ),
              )}
            </tbody>
          </table>
        </div>
      )}

      {view && view.dollar_accounts.length > 0 && <DollarAccounts rows={view.dollar_accounts} onSaved={exitCostSaved} />}

      {closed.length > 0 && (
        <>
          <h3 className="spaced">Posições encerradas</h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  {closedSort.header("ativo", "Ativo")}
                  {closedSort.header("realizado", "Resultado realizado", { numeric: true })}
                </tr>
              </thead>
              <tbody>
                {closed.map((p) => (
                  <tr key={p.asset} className="clickable go"
                  tabIndex={0}
                  onClick={() => onOpen(p.asset)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") onOpen(p.asset);
                  }}
                  title="Abrir o gráfico e os detalhes"
                >
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
