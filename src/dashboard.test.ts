import { describe, expect, it } from "vitest";
import type { DashMonth } from "./api";
import { chained, cumulative, monthTag, periodRange, profitsByYear } from "./dashboard";

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
});
