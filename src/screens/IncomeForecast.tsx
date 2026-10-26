import { useMemo } from "react";
import type { EChartsCoreOption } from "echarts/core";
import type { IncomeRow, MarketIncomes, PortfolioView } from "../api";
import Chart, { axisStyle, base, usePalette } from "../charts/Chart";
import Intro from "../components/Intro";
import { money, monthLabel, price, quantity } from "../format";
import { projection, type Holding } from "../incomes";
import { AssetName } from "./Portfolio";

const brl = (v: number) => money(v.toFixed(2));
const today = () => new Date().toLocaleDateString("sv-SE");

function compact(v: number): string {
  if (Math.abs(v) >= 1e3) return `R$ ${(v / 1e3).toLocaleString("pt-BR", { maximumFractionDigits: 1 })} mil`;
  return `R$ ${v.toLocaleString("pt-BR", { maximumFractionDigits: 0 })}`;
}

/** Lançamentos › Proventos › Próximos 12 meses (estimativa). */
export default function IncomeForecast({
  rows,
  portfolio,
  market,
  loading,
  names,
}: {
  rows: IncomeRow[];
  portfolio: PortfolioView | null;
  market: MarketIncomes | null;
  loading: boolean;
  names: Record<string, string>;
}) {
  const p = usePalette();
  const forecast = useMemo(() => {
    const held: Record<string, number> = {};
    const holdings: Holding[] = [];
    for (const pos of portfolio?.positions ?? []) {
      held[pos.asset] = (held[pos.asset] ?? 0) + Number(pos.quantity);
      if (Number(pos.quantity) > 0) holdings.push({ asset: pos.asset, quantity: Number(pos.quantity), currency: pos.currency });
    }
    const usd = portfolio?.fx ? Number(portfolio.fx.rate) : null;
    return projection(rows, today(), held, holdings, market?.history ?? {}, usd);
  }, [rows, portfolio, market]);

  const chart = useMemo<EChartsCoreOption | null>(() => {
    if (!forecast.total) return null;
    const label = (k: string) => `${monthLabel(Number(k.slice(5, 7))).slice(0, 3).toLowerCase()}/${k.slice(2, 4)}`;
    return {
      ...base(p),
      tooltip: { ...(base(p).tooltip as object), trigger: "axis", valueFormatter: (v: number) => brl(v) },
      legend: { show: false },
      xAxis: { type: "category", data: forecast.months.map(label), ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", ...axisStyle(p), axisLabel: { color: p.muted, formatter: (v: number) => compact(v) } },
      series: [
        {
          name: "Estimativa",
          type: "bar",
          barMaxWidth: 32,
          data: forecast.values,
          itemStyle: { color: p.series[0], opacity: 0.75, borderColor: p.series[0], borderWidth: 1, borderType: "dashed" },
        },
      ],
    };
  }, [forecast, p]);

  if (!portfolio || (!forecast.assets.length && !loading)) return null;
  const fromMarket = forecast.assets.some((a) => a.source === "mercado");

  return (
    <div className="card stack spaced forecast">
      <div>
        <h3>Próximos 12 meses (estimativa)</h3>
        <Intro
          more={
            <>
              Para cada ação, FII e ETF que você tem, a estimativa repete o que o ativo pagou por cota nos últimos 12 meses (pelo Yahoo
              Finance), vezes as cotas que você tem hoje, no mês da data ex. Quando você já lançou os proventos de um ativo nos últimos 12
              meses, valem os seus lançamentos, ajustados pelas cotas de hoje: se comprou mais, a estimativa sobe; se vendeu tudo, ele sai
              da conta. Nos ativos dos EUA, como os ETFs da Nomad, já sai o imposto de 30% retido lá no pagamento, e o valor vai para reais
              pela PTAX de hoje. Na B3, o Yahoo junta dividendos e JCP; o JCP tem 15% retido na fonte, então o líquido pode ser um pouco
              menor. É uma estimativa: as empresas e os fundos podem pagar mais, menos ou em outros meses.
            </>
          }
        >
          Quanto você deve receber nos próximos 12 meses, pelas ações, FIIs e ETFs que tem hoje.
        </Intro>
      </div>
      {loading && !forecast.assets.length ? (
        <p className="muted">Buscando quanto cada ativo pagou nos últimos 12 meses…</p>
      ) : (
        <>
          <div className="cards">
            <div className="card stat">
              <span className="muted">Estimativa para 12 meses</span>
              <strong>{brl(forecast.total)}</strong>
              <span className="muted small">média de {brl(forecast.total / 12)} por mês, líquido</span>
            </div>
            {forecast.received > 0 && (
              <div className="card stat">
                <span className="muted">Recebido nos últimos 12 meses</span>
                <strong>{brl(forecast.received)}</strong>
                <span className="muted small">líquido, pelos seus lançamentos</span>
              </div>
            )}
          </div>
          {chart && <Chart option={chart} height={240} ariaLabel="Estimativa de proventos líquidos por mês" />}
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Ativo</th>
                  <th>Base da estimativa</th>
                  <th className="num">Cotas hoje</th>
                  <th className="num">Estimativa (12 meses)</th>
                </tr>
              </thead>
              <tbody>
                {forecast.assets.map((a) => (
                  <tr key={a.asset} className={a.projected > 0 ? "" : "muted"}>
                    <td>
                      <AssetName asset={a.asset} name={names[a.asset]} />
                    </td>
                    <td>
                      {a.source === "lancados" ? (
                        <>
                          {brl(a.received ?? 0)} <span className="muted small">recebidos (seus lançamentos)</span>
                        </>
                      ) : a.perShare ? (
                        <>
                          {price(String(a.perShare), a.currency)} <span className="muted small">por cota (histórico do ativo)</span>
                        </>
                      ) : (
                        <span className="muted small">não pagou nos últimos 12 meses</span>
                      )}
                    </td>
                    <td className="num">{a.held == null ? "—" : a.held === 0 ? "vendeu tudo" : quantity(String(a.held))}</td>
                    <td className="num">{brl(a.projected)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {loading && <p className="muted small">Buscando o histórico dos outros ativos…</p>}
          {!loading && fromMarket && <p className="muted small">Histórico por cota: Yahoo Finance.</p>}
          {market && market.unavailable.length > 0 && (
            <p className="muted small">Sem histórico no Yahoo para: {market.unavailable.join(", ")}.</p>
          )}
          {forecast.missingFx > 0 && (
            <p className="muted small">{forecast.missingFx} provento(s) em dólar ficaram fora da estimativa por falta da PTAX. Atualize as cotações.</p>
          )}
        </>
      )}
    </div>
  );
}
