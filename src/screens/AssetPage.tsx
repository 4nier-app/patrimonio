import { useEffect, useMemo, useRef, useState } from "react";
import type { ECharts } from "echarts/core";
import type { EChartsCoreOption } from "echarts/core";
import { assetApi, errorMessage, incomesApi, type AssetDetail, type ChartData, type ChartPeriod, type IncomeRow } from "../api";
import { KIND_LABEL as INCOME_KIND, totals } from "../incomes";
import Chart, { axisStyle, base, usePalette } from "../charts/Chart";
import { averageLine, candleLabel, change, operationMarkers } from "../assetChart";
import { averagePrice, date, money, percent, price, quantity, tone } from "../format";
import { resgateNote } from "./Portfolio";

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
  FII: "Fundo imobiliário ou Fiagro",
  ETF: "ETF",
  FI_INFRA: "FI-Infra (isento de IR)",
  ETF_RF: "ETF de renda fixa (IR retido pelo fundo)",
  BDR: "BDR",
  FUNDO: "Fundo de investimento",
  TESOURO: "Tesouro Direto",
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
    case "TESOURO":
      return "Título do Tesouro Direto: um preço por dia útil, publicado pelo Tesouro (preço de recompra, o valor do extrato).";
    default:
      return "Fundos têm uma cota por dia, informada pelo fundo à CVM (normalmente no dia útil seguinte).";
  }
}

function clock(time: number | null): string {
  if (time == null) return "";
  return new Date(time * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" });
}

type Range = { a: number; b: number };

export default function AssetPage({
  ticker,
  refresh,
  onBack,
  backLabel = "Voltar",
  onLoaded,
}: {
  ticker: string;
  refresh: number;
  onBack: () => void;
  backLabel?: string;
  /** Chamado quando os dados do ativo chegam (ex.: lista de vistos recentemente). */
  onLoaded?: (d: AssetDetail) => void;
}) {
  const loaded = useRef(onLoaded);
  loaded.current = onLoaded;
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
        loaded.current?.(d);
        // Fundo não tem gráfico do dia: começa em 1 ano.
        if (d.venue === "FUNDO" || d.venue === "TESOURO") setPeriod((x) => (x === "1d" || x === "5d" ? "1y" : x));
      })
      .catch((e) => setError(errorMessage(e)));
  }, [ticker, refresh]);

  // Entre chaves: no Windows, window.scrollTo devolve uma Promise, e o React
  // tentaria usá-la como função de limpeza ao fechar a página (tela branca).
  useEffect(() => {
    window.scrollTo({ top: 0 });
  }, [ticker]);

  // Proventos recebidos deste ativo.
  const [incomes, setIncomes] = useState<IncomeRow[]>([]);
  useEffect(() => {
    incomesApi
      .load()
      .then((v) => setIncomes(v.items.filter((r) => r.income.asset === ticker)))
      .catch(() => setIncomes([]));
  }, [ticker, refresh]);

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
    // Fundo e Tesouro: um preço por dia.
    const fund = detail.venue === "FUNDO" || detail.venue === "TESOURO";
    const unit = detail.venue === "TESOURO" ? "Preço" : "Cota";
    const fullLabel = (t: number) => {
      const iso = new Date((t + off) * 1000).toISOString();
      const d = `${iso.slice(8, 10)}/${iso.slice(5, 7)}/${iso.slice(0, 4)}`;
      return intraday ? `${d} às ${iso.slice(11, 16)}` : d;
    };
    const fmt = (v: number) => v.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: decimals });
    const priceSeries = lineOnly
      ? {
          id: "price",
          name: "Preço",
          type: "line",
          data: candles.map((c) => c.close),
          showSymbol: false,
          lineStyle: { width: 2, color: p.series[0] },
          itemStyle: { color: p.series[0] },
          areaStyle: { color: p.series[0], opacity: 0.08 },
        }
      : {
          id: "price",
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
        formatter: (params: unknown) => {
          const list = params as { dataIndex: number }[];
          const i = list[0]?.dataIndex;
          if (i == null || !candles[i]) return "";
          const c = candles[i];
          const prev = i > 0 ? candles[i - 1].close : null;
          const var_ = prev ? c.close / prev - 1 : null;
          const avgHere = intraday ? avgNow : avg[i];
          const rows: [string, string][] = lineOnly
            ? [[fund ? unit : "Fechamento", fmt(c.close)]]
            : [
                ["Abertura", fmt(c.open)],
                ["Máxima", fmt(c.high)],
                ["Mínima", fmt(c.low)],
                ["Fechamento", fmt(c.close)],
              ];
          if (var_ != null) rows.push(["Variação", `${var_ >= 0 ? "+" : ""}${(var_ * 100).toLocaleString("pt-BR", { maximumFractionDigits: 2 })}%`]);
          if (!lineOnly && c.volume > 0) rows.push(["Volume", c.volume.toLocaleString("pt-BR", { maximumFractionDigits: 0 })]);
          if (avgHere != null) rows.push(["Seu preço médio", fmt(avgHere)]);
          for (const m of markers.filter((m) => m.index === i)) rows.push([m.side === "COMPRA" ? "Sua compra" : "Sua venda", fmt(m.price)]);
          return `<div style="font-weight:600;margin-bottom:4px">${fullLabel(c.time)}</div>` +
            rows.map(([k, v]) => `<div style="display:flex;justify-content:space-between;gap:16px"><span>${k}</span><strong>${v}</strong></div>`).join("");
        },
      },
      xAxis: { type: "category", data: labels, boundaryGap: !lineOnly, ...axisStyle(p), splitLine: { show: false } },
      yAxis: { type: "value", scale: true, position: "right", ...axisStyle(p), axisLabel: { color: p.muted, formatter: fmt } },
      // Arrastar o mouse mede um trecho; a roda e a barra de baixo dão zoom.
      dataZoom:
        candles.length > 80
          ? [{ type: "inside", moveOnMouseMove: false }, { type: "slider", height: 18, bottom: 8 }]
          : [{ type: "inside", moveOnMouseMove: false }],
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
  // Medir um trecho: clicar e arrastar no gráfico mostra a variação entre os dois pontos.
  const echart = useRef<ECharts | null>(null);
  const [range, setRange] = useState<Range | null>(null);
  const dragging = useRef<number | null>(null);
  const candlesRef = useRef(candles);
  candlesRef.current = candles;
  useEffect(() => {
    setRange(null);
  }, [option]);

  function paintRange(r: Range | null) {
    const c = echart.current;
    if (!c || c.isDisposed()) return;
    const cs = candlesRef.current;
    const [a, b] = r ? [Math.min(r.a, r.b), Math.max(r.a, r.b)] : [0, 0];
    const gain = r ? cs[b].close >= cs[a].close : true;
    c.setOption({
      series: [
        {
          id: "price",
          markArea: r
            ? {
                silent: true,
                itemStyle: { color: gain ? p.series[2] : p.neg, opacity: 0.12 },
                data: [[{ xAxis: a }, { xAxis: b }]],
              }
            : { data: [] },
        },
      ],
    });
  }

  function onReady(c: ECharts | null) {
    echart.current = c;
    if (!c) return;
    const zr = c.getZr();
    const indexAt = (x: number, y: number): number | null => {
      if (!c.containPixel("grid", [x, y])) return null;
      const v = c.convertFromPixel({ xAxisIndex: 0 }, x) as unknown as number;
      const n = candlesRef.current.length;
      if (!n || v == null || Number.isNaN(Number(v))) return null;
      return Math.max(0, Math.min(n - 1, Math.round(Number(v))));
    };
    zr.on("mousedown", (e) => {
      const i = indexAt(e.offsetX, e.offsetY);
      if (i == null) return;
      dragging.current = i;
      const r = { a: i, b: i };
      setRange(r);
      paintRange(r);
    });
    zr.on("mousemove", (e) => {
      if (dragging.current == null) return;
      const i = indexAt(e.offsetX, e.offsetY);
      if (i == null) return;
      const r = { a: dragging.current, b: i };
      setRange(r);
      paintRange(r);
    });
    zr.on("mouseup", () => {
      dragging.current = null;
    });
    zr.on("globalout", () => {
      dragging.current = null;
    });
  }

  const measured = (() => {
    if (!range || range.a === range.b || !candles[range.a] || !candles[range.b]) return null;
    const [a, b] = [Math.min(range.a, range.b), Math.max(range.a, range.b)];
    const from = candles[a];
    const to = candles[b];
    const pct = to.close / from.close - 1;
    return {
      from: candleLabel(from.time, off, period === "1d" ? "1d" : period === "5d" ? "5d" : "1y"),
      to: candleLabel(to.time, off, period === "1d" ? "1d" : period === "5d" ? "5d" : "1y"),
      pct,
      diff: to.close - from.close,
      start: from.close,
      end: to.close,
    };
  })();

  const td = detail?.venue === "TESOURO";
  const fund = detail?.venue === "FUNDO" || td;
  const title = detail ? (fund ? detail.name || detail.ticker : detail.ticker) : ticker;
  const subtitle = detail ? (td ? "Tesouro Direto" : fund ? `CNPJ ${detail.ticker}` : detail.name) : "";

  return (
    <section className="asset-page">
      <button className="link" onClick={onBack}>
        ← {backLabel}
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
          <Chart option={option} height={380} ariaLabel={`Gráfico de preços de ${title}`} onReady={onReady}>
            {measured && (
              <div className="range-badge" role="status">
                <span className="muted">
                  {measured.from} → {measured.to}
                </span>
                <strong className={tone(String(measured.pct))}>
                  {measured.pct >= 0 ? "+" : ""}
                  {percent(String(measured.pct))}
                </strong>
                <span className={tone(String(measured.diff))}>{money(measured.diff.toFixed(2), cur)}</span>
                <button
                  className="link"
                  aria-label="Limpar a medição"
                  onClick={() => {
                    setRange(null);
                    paintRange(null);
                  }}
                >
                  ✕
                </button>
              </div>
            )}
          </Chart>
        ) : (
          <p className="muted chart-empty">
            {loading || !chart
              ? "Carregando o gráfico…"
              : td && !detail?.history.length
                ? "Os preços do Tesouro são baixados só para os títulos que você lança em Lançamentos. Lance uma compra deste título para ver o gráfico."
                : fund && !detail?.history.length
                ? "As cotas de um fundo são baixadas da CVM só para os fundos que você lança em Lançamentos. Lance uma aplicação neste fundo para ver o gráfico."
                : "Sem preços para este período."}
          </p>
        )}
        {intraday && avgOutside && pos && (
          <p className="muted small">
            Seu preço médio ({averagePrice(pos.average_price, cur)}) está {avgOutside === "abaixo" ? "abaixo" : "acima"} da faixa de preços do
            período, por isso a linha não aparece no gráfico.
          </p>
        )}
        {detail && (
          <p className="muted small">
            Clique e arraste no gráfico para ver a variação entre dois pontos; use a roda do mouse para aproximar.{" "}
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
            <span className="muted">{td ? "Títulos" : fund ? "Cotas" : "Quantidade"}</span>
            <strong>{quantity(pos.quantity)}</strong>
          </div>
          <div className="card stat">
            <span className="muted">{fund && !td ? "Valor médio da cota" : "Preço médio"}</span>
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
                {cur === "USD" && resgateNote(pos) && (
                  <span title={resgateNote(pos)!}>
                    {" "}
                    valor de resgate <span className="info-mark">ⓘ</span>
                  </span>
                )}
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

      {incomes.length > 0 && (
        <>
          <h3 className="spaced">
            Proventos recebidos <span className="muted small">· {money(totals(incomes).net.toFixed(2))} líquidos no total</span>
          </h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Pagamento</th>
                  <th>Tipo</th>
                  <th>Conta</th>
                  <th className="num">Por cota</th>
                  <th className="num">Líquido</th>
                </tr>
              </thead>
              <tbody>
                {incomes.map((r) => (
                  <tr key={r.income.id}>
                    <td>{date(r.income.pay_date)}</td>
                    <td>{INCOME_KIND[r.income.kind]}</td>
                    <td>{r.income.account}</td>
                    <td className="num">{r.per_share ? price(r.per_share, r.income.currency) : "—"}</td>
                    <td className="num">{money(r.net, r.income.currency)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {detail && detail.lots.length > 0 && (
        <>
          <h3 className="spaced">{fund ? "Aplicações em aberto" : "Compras em aberto"}</h3>
          <div className="scroll">
            <table>
              <thead>
                <tr>
                  <th>Data</th>
                  <th>Conta</th>
                  <th className="num">{td ? "Títulos" : fund ? "Cotas" : "Qtd."}</th>
                  <th className="num">Custo por {td ? "título" : fund ? "cota" : "ação"}</th>
                  <th className="num">Custo</th>
                  <th className="num">Valor hoje</th>
                  <th className="num" colSpan={2}>
                    Ganho desde a compra
                  </th>
                </tr>
              </thead>
              <tbody>
                {detail.lots.map((l, k) => {
                  const pct = l.gain != null && Number(l.cost) ? String(Number(l.gain) / Number(l.cost)) : null;
                  const pctBrl = l.gain_brl != null && Number(l.cost_brl) ? String(Number(l.gain_brl) / Number(l.cost_brl)) : null;
                  return (
                    <tr key={`${l.id}:${l.account}:${k}`}>
                      <td>{date(l.date)}</td>
                      <td>{l.account}</td>
                      <td className="num">
                        {quantity(l.quantity)}
                        {Number(l.quantity) !== Number(l.bought) && <div className="muted small">de {quantity(l.bought)}</div>}
                      </td>
                      <td className="num" title={`Valor exato: ${l.unit_cost}`}>{averagePrice(l.unit_cost, cur)}</td>
                      <td className="num">
                        {money(l.cost, cur)}
                        {cur === "USD" && <div className="muted small">{money(l.cost_brl)}</div>}
                      </td>
                      <td className="num">
                        {money(l.value, cur)}
                        {cur === "USD" && <div className="muted small">{money(l.value_brl)}</div>}
                      </td>
                      <td className={`num ${tone(l.gain)}`}>
                        {money(l.gain, cur)}
                        {cur === "USD" && <div className={`small ${tone(l.gain_brl)}`}>{money(l.gain_brl)}</div>}
                      </td>
                      <td className={`num ${tone(pct)}`}>
                        {pct != null ? percent(pct) : "—"}
                        {cur === "USD" && pctBrl != null && <div className={`small ${tone(pctBrl)}`}>{percent(pctBrl)}</div>}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
          <p className="muted small">
            O que resta de cada {fund ? "aplicação" : "compra"}, com as taxas no custo, e quanto ela ganhou ou perdeu até a cotação de hoje. Nas vendas,{" "}
            {fund ? "as aplicações" : "as compras"} mais antigas saem primeiro; numa transferência de custódia, a quantidade muda de conta
            mantendo a data e o custo. É só informativo: o preço médio, o resultado e o imposto de renda continuam calculados pelo preço médio, como
            manda a Receita.
          </p>
        </>
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
                  <th className="num">{td ? "Títulos" : fund ? "Cotas" : "Qtd."}</th>
                  <th className="num">{fund && !td ? "Valor da cota" : "Preço"}</th>
                  <th className="num">Valor</th>
                  <th className="num">{fund ? "IR/IOF" : "Taxas"}</th>
                </tr>
              </thead>
              <tbody>
                {detail.operations.map((x) => {
                  const o = x.operation;
                  const event = o.side === "AJUSTE";
                  const label = event
                    ? (o.note ?? "Evento")
                    : o.side === "TRANSFERENCIA"
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
                      <td className="num">
                        {event && Number(o.quantity) > 0 ? "+" : ""}
                        {quantity(o.quantity)}
                      </td>
                      <td className="num">{o.side === "TRANSFERENCIA" || event ? "—" : price(o.price, cur)}</td>
                      <td className="num">{o.side === "TRANSFERENCIA" || (event && Number(x.gross) === 0) ? "—" : money(x.gross, cur)}</td>
                      <td className="num">{event ? "—" : money(x.fees, cur)}</td>
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
