import { describe, expect, it } from "vitest";
import type { Candle, PricedOperation } from "./api";
import { averageLine, candleLabel, change, localDate, operationMarkers } from "./assetChart";

// 02/03/2026 13:00 UTC = 10:00 em Brasília.
const T = Date.UTC(2026, 2, 2, 13, 0) / 1000;
const OFF = -10800;
const DAY = 86400;
const candle = (time: number, close = 10): Candle => ({ time, open: close, high: close, low: close, close, volume: 0 });

const op = (date: string, side: "COMPRA" | "VENDA", price: string): PricedOperation => ({
  operation: {
    id: date + side,
    account: "X",
    asset: "TEST3",
    date,
    side,
    quantity: "10",
    price,
    manual_fees: null,
    note: null,
    currency: "BRL",
    from_account: null,
    residue: null,
  },
  gross: "0",
  fees: "0",
  fee_origin: "automática",
});

describe("gráfico do ativo", () => {
  it("usa a hora da bolsa", () => {
    expect(localDate(T, OFF)).toBe("2026-03-02");
    expect(candleLabel(T, OFF, "1d")).toBe("10:00");
    expect(candleLabel(T, OFF, "5d")).toBe("02/03 10:00");
    expect(candleLabel(T, OFF, "1y")).toBe("02/03/26");
    expect(candleLabel(T, OFF, "max")).toBe("03/2026");
  });

  it("preço médio só enquanto há posição", () => {
    const candles = [candle(T - DAY), candle(T), candle(T + DAY), candle(T + 2 * DAY)];
    const steps = [
      { date: "2026-03-02", quantity: "10", average_price: "9.5" },
      { date: "2026-03-04", quantity: "0", average_price: "0" },
    ];
    expect(averageLine(candles, OFF, steps)).toEqual([null, 9.5, 9.5, null]);
  });

  it("marca compras e vendas na vela do dia", () => {
    const candles = [candle(T), candle(T + 3 * DAY)];
    const ops = [op("2026-03-01", "COMPRA", "9"), op("2026-03-03", "VENDA", "11"), op("2026-03-02", "COMPRA", "10")];
    const m = operationMarkers(candles, OFF, ops, false);
    expect(m.map((x) => [x.index, x.side])).toEqual([
      [1, "VENDA"],
      [0, "COMPRA"],
    ]);
    expect(operationMarkers(candles, OFF, ops, true).map((x) => x.index)).toEqual([0]);
    expect(change(11, 10)).toBeCloseTo(0.1);
    expect(change(11, null)).toBeNull();
  });
});
