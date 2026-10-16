import { useEffect, useMemo, useState } from "react";
import type { EChartsCoreOption } from "echarts/core";
import { assetApi, errorMessage, type AssetDetail, type ChartData, type ChartPeriod } from "../api";
import Chart, { axisStyle, base, usePalette } from "../charts/Chart";
import { averageLine, candleLabel, change, operationMarkers } from "../assetChart";
import { averagePrice, date, money, percent, price, quantity, tone } from "../format";

const PERIODS: { id: ChartPeriod; label: string }[] = [
  { id: "1d", label: "Dia" },
  { id: "5d", label: "5 dias" },
  { id: "1mo", label: "1 mês" },
  { id: "6mo", label: "6 meses" },
  { id: "1y", label: "1 ano" },
  { id: "5y", label: "5 anos" },
  { id: "max", label: "Tudo" },
];

const INTERVALS = [
  { id: "1m", label: "1 min" },
  { id: "5m", label: "5 min" },
  { id: "15m", label: "15 min" },
];

const KIND_LABEL: Record<string, string> = {
  ACAO: "Ação",
  FII: "Fundo imobiliário",
  ETF: "ETF",
  BDR: "BDR",
  FUNDO: "Fundo de investimento",
  CRIPTO: "Criptomoeda",
};

/** Intervalo de atualização automática no gráfico do dia. */
const REFRESH_MS = 60_000;

function delayNote(d: AssetDetail): string {
  switch (d.venue) {
    case "B3":
      return "Cotação da B3 com cerca de 15 minutos de atraso (a cotação ao vivo da B3 é paga).";
    case "US":
      return "Cotação dos EUA quase em tempo real durante o pregão americano.";
    case "CRIPTO":
      return "Cotação em tempo real (Coinbase), em dólar.";
    default:
      return "Fundos têm uma cota por dia, informada pelo fundo à CVM (normalmente no dia útil seguinte).";
  }
}

function clock(time: number | null): string {
  if (time == null) return "";
  return new Date(time * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
}

export default function AssetPage({ ticker, refresh, onBack }: { ticker: string; refresh: number; onBack: () => void }) {
  const p = usePalette();
  const [detail, setDetail] = useState<AssetDetail | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [period, setPeriod] = useState<ChartPeriod>("1d");
  const [interval, setIntervalId] = useState("5m");
  const [chart, setChart] = useState<ChartData | null>(null);
  const [loading, setLoading] = useState(false);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    assetApi
      .detail(ticker)
      .then((d) => {
        setDetail(d);
        // Fundo não tem gráfico do dia: começa em 1 ano.
        if (d.venue === "FUNDO") setPeriod((x) => (x === "1d" || x === "5d" ? "1y" : x));
      })
      .catch((e) => setError(errorMessage(e)));
  }, [ticker, refresh]);

  useEffect(() => window.scrollTo({ top: 0 }), [ticker]);

  useEffect(() => {
    if (!detail) return;
    let alive = true;
    setLoading(true);
    assetApi
      .chart(ticker, period, interval)
      .then((c) => alive && setChart(c))
      .catch((e) => alive && setError(errorMessage(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, [detail, ticker, period, interval, tick]);

  // No dia e em 5 dias, atualiza sozinho a cada minuto enquanto a janela está visível.
  useEffect(() => {
    if (period !== "1d" && period !== "5d") return;
    const id = window.setInterval(() => {
      if (document.visibilityState === "visible") setTick((n) => n + 1);
    }, REFRESH_MS);
    return () => window.clearInterval(id);
  }, [period]);

  const intraday = period === "1d" || period === "5d";
  const candles = chart?.candles ?? [];
  const off = chart?.gmtoffset ?? 0;
  const lineOnly = !chart || chart.source === "cofre";
  const last = chart?.price ?? (candles.length ? candles[candles.length - 1].close : null);
  const baseline = period === "1d" ? chart?.previous_close ?? candles[0]?.open ?? null : candles[0]?.open ?? null;
  const delta = change(last, baseline);
  const cur = detail?.currency ?? "BRL";
  const pos = detail?.position;

  const option = useMemo<EChartsCoreOption | null>(() => {
    if (!detail || !candles.length) return null;
    const labels = candles.map((c) => candleLabel(c.time, off, period));
    // No intradiário o preço médio é uma linha fixa que não muda a escala do
    // gráfico (se estiver fora da faixa do dia, o aviso fica embaixo).
    const avg = intraday ? candles.map(() => null) : averageLine(candles, off, detail.average_steps);
    const avgNow = pos && Number(pos.quantity) > 0 ? Number(pos.average_price) : null;
    const markers = operationMarkers(candles, off, detail.operations, intraday);
    const up = p.series[2];
    const down = p.neg;
    const decimals = cur === "USD" || detail.venue === "FUNDO" ? 4 : 2;
    const fmt = (v: number) => v.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: decimals });
    const priceSeries = lineOnly
      ? {
          name: "Preço",
          type: "line",
          data: candles.map((c) => c.close),
          showSymbol: false,
          lineStyle: { width: 2, color: p.series[0] },
          itemStyle: { color: p.series[0] },
          areaStyle: { color: p.series[0], opacity: 0.08 },
        }
      : {
          name: "Preço",
          type: "candlestick",
          data: candles.map((c) => [c.open, c.close, c.low, c.high]),
          itemStyle: { color: up, color0: down, borderColor: up, borderColor0: down },
        };
    if (intraday && avgNow != null) {
      Object.assign(priceSeries, {
        markLine: {
          symbol: "none",
          silent: true,
          lineStyle: { type: "dashed", color: p.series[3], width: 1.5 },
          label: { formatter: `Seu preço médio ${fmt(avgNow)}`, position: "insideStartTop", color: p.series[3] },
          data: [{ yAxis: avgNow }],
        },
      });
    }
    return {
      ...base(p),
      grid: { left: 8, right: 16, top: 36, bottom: candles.length > 80 ? 48 : 16, containLabel: true },
      legend: {
        ...(base(p).legend as object),
        data: [
          "Preço",
          ...(avg.some((x) => x != null) ? ["Seu preço médio"] : []),
          ...(markers.some((m) => m.side === "COMPRA") ? ["Compras"] : []),
          ...(markers.some((m) => m.side === "VENDA") ? ["Vendas"] : []),
        ],
      },
      tooltip: {
        ...(base(p).tooltip as object),
        trigger: "axis",
        axisPointer: { type: "cross" },
        valueFormatter: (v: unknown) => (Array.isArray(v) ? v.map((x) => fmt(Number(x))).join(" / ") : v == null ? "—" : fmt(Number(v))),
      },
      xAxis: { type: "category", data: labels, boundaryGap: !lineOnly, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", scale: true, position: "right", ...axisStyle(p), axisLabel: { color: p.muted, formatter: fmt } },
      dataZoom: candles.length > 80 ? [{ type: "inside" }, { type: "slider", height: 18, bottom: 8 }] : [{ type: "inside" }],
      series: [
        priceSeries,
        {
          name: "Seu preço médio",
          type: "line",
          step: "end",
          data: avg,
          showSymbol: false,
          connectNulls: false,
          lineStyle: { width: 1.5, type: "dashed", color: p.series[3] },
          itemStyle: { color: p.series[3] },
        },
        {
          name: "Compras",
          type: "scatter",
          symbol: "triangle",
          symbolSize: 12,
          itemStyle: { color: up, borderColor: p.surface, borderWidth: 1 },
          data: markers.filter((m) => m.side === "COMPRA").map((m) => [m.index, m.price]),
          tooltip: { show: false },
        },
        {
          name: "Vendas",
          type: "scatter",
          symbol: "triangle",
          symbolRotate: 180,
          symbolSize: 12,
          itemStyle: { color: down, borderColor: p.surface, borderWidth: 1 },
          data: markers.filter((m) => m.side === "VENDA").map((m) => [m.index, m.price]),
          tooltip: { show: false },
        },
      ],
    };
  }, [detail, candles, off, period, intraday, lineOnly, p, cur, pos]);

  const lows = candles.map((c) => c.low);
  const highs = candles.map((c) => c.high);
  const avgValue = pos && Number(pos.quantity) > 0 ? Number(pos.average_price) : null;
  const avgOutside =
    avgValue == null || !candles.length
      ? null
      : avgValue < Math.min(...lows)
        ? "abaixo"
        : avgValue > Math.max(...highs)
          ? "acima"
          : null;
  const fund = detail?.venue === "FUNDO";
  const title = detail ? (fund ? detail.name || detail.ticker : detail.ticker) : ticker;
  const subtitle = detail ? (fund ? `CNPJ ${detail.ticker}` : detail.name) : "";

  return (
    <section className="asset-page">
      <button className="link" onClick={onBack}>
        ← Voltar para a Carteira
      </button>
      {error && <p className="error">{error}</p>}
      <div className="asset-head">
        <div>
          <h2>{title}</h2>
          <p className="muted">
            {subtitle}
            {detail && KIND_LABEL[detail.kind] && <> · {KIND_LABEL[detail.kind]}</>}
          </p>
        </div>
        {last != null && (
          <div className="asset-price">
            <strong>{price(String(last), cur)}</strong>
            {delta != null && (
              <span className={tone(String(delta))}>
                {delta > 0 ? "▲" : delta < 0 ? "▼" : ""} {percent(String(delta))} {period === "1d" ? "no dia" : "no período"}
              </span>
            )}
          </div>
        )}
      </div>

      <div className="card">
        <div className="asset-controls">
          <div className="segmented" role="group" aria-label="Período">
            {PERIODS.filter((x) => !fund || (x.id !== "1d" && x.id !== "5d")).map((x) => (
              <button key={x.id} className={period === x.id ? "seg active" : "seg"} onClick={() => setPeriod(x.id)}>
                {x.label}
              </button>
            ))}
          </div>
          {period === "1d" && !fund && (
            <div className="segmented small" role="group" aria-label="Tamanho da vela">
              {INTERVALS.map((x) => (
                <button key={x.id} className={interval === x.id ? "seg active" : "seg"} onClick={() => setIntervalId(x.id)}>
                  {x.label}
                </button>
              ))}
            </div>
          )}
        </div>
        {option ? (
          <Chart option={option} height={380} ariaLabel={`Gráfico de preços de ${title}`} />
        ) : (
          <p className="muted chart-empty">{loading || !chart ? "Carregando o gráfico…" : "Sem preços para este período."}</p>
        )}
        {intraday && avgOutside && pos && (
          <p className="muted small">
            Seu preço médio ({averagePrice(pos.average_price, cur)}) está {avgOutside === "abaixo" ? "abaixo" : "acima"} da faixa de preços do
            período, por isso a linha não aparece no gráfico.
          </p>
        )}
        {detail && (
          <p className="muted small">
            {delayNote(detail)}
            {chart?.source && chart.source !== "cofre" && (
              <>
                {" "}
                Fonte: {chart.source}
                {intraday && chart.price_time ? `, último negócio às ${clock(chart.price_time)}` : ""}.
                {intraday && " Atualiza sozinho a cada minuto."}
              </>
            )}
            {chart?.offline && <> Sem acesso à fonte agora ({chart.offline}); mostrando os fechamentos guardados no aplicativo.</>}
            {chart?.source === "cofre" && intraday && " O gráfico do dia precisa de internet."}
          </p>
        )}
      </div>

      {pos && (
        <div className="cards">
          <div className="card stat">
            <span className="muted">{fund ? "Cotas" : "Quantidade"}</span>
            <strong>{quantity(pos.quantity)}</strong>
          </div>
          <div className="card stat">
            <span className="muted">{fund ? "Valor médio da cota" : "Preço médio"}</span>
            <strong title={`Valor exato: ${pos.average_price}`}>
              {fund ? price(pos.average_price, cur) : averagePrice(pos.average_price, cur)}
            </strong>
          </div>
          <div className="card stat">
            <span className="muted">Custo</span>
            <strong>{money(pos.cost, cur)}</strong>
            {cur === "USD" && <span className="muted small">{money(pos.cost_brl)}</span>}
          </div>
          <div className="card stat">
            <span className="muted">Valor atual</span>
            <strong>{money(pos.market_value, cur)}</strong>
            {pos.quote && (
              <span className="muted small">
                cotação de {date(pos.quote.date)}
                {cur === "USD" && pos.market_value_brl != null && ` · ${money(pos.market_value_brl)}`}
              </span>
            )}
          </div>
          <div className="card stat">
            <span className="muted">Resultado em aberto</span>
            <strong className={tone(pos.unrealized)}>
              {money(pos.unrealized, cur)}{" "}
              {pos.unrealized != null && Number(pos.cost) > 0 && <small>{percent(String(Number(pos.unrealized) / Number(pos.cost)))}</small>}
            </strong>
          </div>
          <div className="card stat">
            <span className="muted">Resultado realizado</span>
            <strong className={tone(pos.realized)}>{money(pos.realized, cur)}</strong>
            {fund && <span className="muted small">já descontado o IR e o IOF</span>}
          </div>
        </div>
      )}

      {detail && detail.operations.length > 0 && (
        <>
          <h3 className="spaced">Seus lançamentos</h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Data</th>
                  <th>Conta</th>
                  <th>Operação</th>
                  <th className="num">{fund ? "Cotas" : "Qtd."}</th>
                  <th className="num">{fund ? "Valor da cota" : "Preço"}</th>
                  <th className="num">Valor</th>
                  <th className="num">{fund ? "IR/IOF" : "Taxas"}</th>
                </tr>
              </thead>
              <tbody>
                {detail.operations.map((x) => {
                  const o = x.operation;
                  const label =
                    o.side === "TRANSFERENCIA"
                      ? "Transferência"
                      : fund
                        ? o.note === "Come-cotas"
                          ? "Come-cotas"
                          : o.side === "COMPRA"
                            ? "Aplicação"
                            : "Resgate"
                        : o.side === "COMPRA"
                          ? "Compra"
                          : "Venda";
                  return (
                    <tr key={o.id}>
                      <td>{date(o.date)}</td>
                      <td>{o.account}</td>
                      <td>{label}</td>
                      <td className="num">{quantity(o.quantity)}</td>
                      <td className="num">{o.side === "TRANSFERENCIA" ? "—" : price(o.price, cur)}</td>
                      <td className="num">{o.side === "TRANSFERENCIA" ? "—" : money(x.gross, cur)}</td>
                      <td className="num">{money(x.fees, cur)}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </>
      )}
    </section>
  );
}
