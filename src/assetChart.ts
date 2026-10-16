import type { AverageStep, Candle, ChartPeriod, PricedOperation } from "./api";

/** Data local da bolsa ("2026-03-02") de um instante em segundos. */
export function localDate(time: number, gmtoffset: number): string {
  return new Date((time + gmtoffset) * 1000).toISOString().slice(0, 10);
}

/** Rótulo do eixo: hora no dia, dia e hora em 5 dias, data nos demais. */
export function candleLabel(time: number, gmtoffset: number, period: ChartPeriod): string {
  const iso = new Date((time + gmtoffset) * 1000).toISOString();
  const [y, m, d] = iso.slice(0, 10).split("-");
  const hm = iso.slice(11, 16);
  if (period === "1d") return hm;
  if (period === "5d") return `${d}/${m} ${hm}`;
  if (period === "5y" || period === "max") return `${m}/${y}`;
  return `${d}/${m}/${y.slice(2)}`;
}

/** Preço médio em cada vela (vazio antes da primeira compra ou sem posição). */
export function averageLine(candles: Candle[], gmtoffset: number, steps: AverageStep[]): (number | null)[] {
  return candles.map((c) => {
    const day = localDate(c.time, gmtoffset);
    let current: AverageStep | null = null;
    for (const s of steps) {
      if (s.date <= day) current = s;
      else break;
    }
    return current && Number(current.quantity) > 0 ? Number(current.average_price) : null;
  });
}

export type Marker = { index: number; price: number; side: "COMPRA" | "VENDA"; label: string };

/**
 * Compras e vendas no gráfico: cada lançamento cai na primeira vela do seu dia
 * (ou do primeiro dia seguinte com pregão). Lançamentos fora do período ficam
 * de fora; no intradiário, só os do mesmo dia.
 */
export function operationMarkers(candles: Candle[], gmtoffset: number, ops: PricedOperation[], intraday: boolean): Marker[] {
  if (!candles.length) return [];
  const days = candles.map((c) => localDate(c.time, gmtoffset));
  const out: Marker[] = [];
  for (const p of ops) {
    const o = p.operation;
    if (o.side === "TRANSFERENCIA") continue;
    const index = intraday ? days.indexOf(o.date) : days.findIndex((d) => d >= o.date);
    if (index < 0 || (!intraday && o.date < days[0])) continue;
    out.push({ index, price: Number(o.price), side: o.side, label: `${o.side === "COMPRA" ? "Compra" : "Venda"} de ${o.quantity}` });
  }
  return out;
}

/** Variação do último preço sobre a base (fechamento anterior ou início do período). */
export function change(last: number | null, base: number | null): number | null {
  if (last == null || base == null || base === 0) return null;
  return last / base - 1;
}
