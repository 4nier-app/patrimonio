import { describe, expect, it } from "vitest";
import type { DashboardView, DashMonth } from "./api";
import { chained, cumulative, dayFrame, monthFrame, monthTag, periodRange, profitsByYear, singleMonth } from "./dashboard";

const m = (year: number, month: number, rate: string | null, profit: string | null = "10"): DashMonth => ({
  year,
  month,
  period_start: "",
  period_end: "",
  value: "0",
  invested: "0",
  profit,
  rate,
  drawdown: null,
  explanation: [],
});

describe("painel", () => {
  it("recorta o período", () => {
    const months = Array.from({ length: 15 }, (_, i) => m(2025 + Math.floor((i + 9) / 12), ((i + 9) % 12) + 1, "0"));
    expect(periodRange(months, "inicio")).toEqual([0, 15]);
    expect(periodRange(months, "12m")).toEqual([3, 15]);
    // Meses de 2026: a partir de jan/26 (índice 3).
    expect(months[3].year).toBe(2026);
    expect(periodRange(months, "ano")).toEqual([3, 15]);
    // Meses escolhidos: um só, um intervalo (em qualquer ordem) e fora dos dados.
    expect(periodRange(months, { from: "2026-02", to: "2026-02" })).toEqual([4, 5]);
    expect(periodRange(months, { from: "2026-03", to: "2025-11" })).toEqual([1, 6]);
    expect(periodRange(months, { year: 2025 })).toEqual([0, 3]);
    expect(periodRange(months, { from: "2030-01", to: "2030-05" })).toEqual([15, 15]);
  });

  it("encadeia e interrompe quando falta um mês", () => {
    expect(cumulative(["0.1", "-0.1"])[1]).toBeCloseTo(-0.01);
    expect(cumulative(["0.1", null, "0.1"])).toEqual([expect.closeTo(0.1), null, null]);
    expect(chained([])).toBeNull();
  });

  it("soma lucros por ano e rotula meses", () => {
    expect(profitsByYear([m(2025, 12, "0", "5"), m(2026, 1, "0", "7"), m(2026, 2, "0", "-2")])).toEqual([
      { year: 2025, profit: 5 },
      { year: 2026, profit: 5 },
    ]);
    expect(monthTag({ year: 2026, month: 3 })).toBe("mar/26");
  });

  it("mês sem carteira fica em branco sem interromper o acumulado", () => {
    expect(cumulative(["0.1", undefined, "0.1"])).toEqual([expect.closeTo(0.1), null, expect.closeTo(0.21)]);
  });

  it("um ano escolhido tem os 12 meses", () => {
    const view = {
      months: [m(2025, 11, "0.01"), m(2026, 3, "0.02"), m(2026, 4, "0.03")],
      benchmarks: [{ id: "CDI", name: "CDI", kind: "DAILY_RATE", source: "", rates: ["0.1", "0.2", "0.3"], same_flows: ["1", "2", "3"], explanations: [] }],
      contributions: [{ asset: "ABCD3", profits: ["1", "2", null] }],
    } as unknown as DashboardView;
    const f = monthFrame(view, { year: 2026 });
    expect(f.labels).toHaveLength(12);
    expect(f.labels[0]).toBe("jan/26");
    expect(f.slots.map((s) => s?.month ?? null)).toEqual([null, null, 3, 4, null, null, null, null, null, null, null, null]);
    expect(f.bench[0].rates.slice(1, 5)).toEqual([undefined, "0.2", "0.3", undefined]);
    expect(f.contributions[0].profits.slice(2, 4)).toEqual(["2", null]);
    expect(monthFrame(view, "inicio").slots).toHaveLength(3);
  });

  it("um mês sozinho vira dias", () => {
    expect(singleMonth({ from: "2026-03", to: "2026-03" })).toEqual({ year: 2026, month: 3 });
    expect(singleMonth({ from: "2026-02", to: "2026-03" })).toBeNull();
    expect(singleMonth("12m")).toBeNull();
    const day = { ...m(2026, 3, "0.001"), period_end: "2026-03-04" };
    const f = dayFrame({ days: [day], benchmarks: [], contributions: [] });
    expect(f.days).toBe(true);
    expect(f.labels).toEqual(["04/03"]);
  });
});
