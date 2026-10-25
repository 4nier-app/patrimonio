import { describe, expect, it } from "vitest";
import { progress } from "./goal";

describe("meta de patrimônio", () => {
  const now = { date: "2026-10-25", value: 200_000 };

  it("calcula quanto falta e quanto por mês até a data", () => {
    const p = progress(now, { value: 300_000, date: "2028-10-25" }, []);
    expect(p.missing).toBe(100_000);
    expect(p.done).toBeCloseTo(2 / 3, 6);
    expect(p.monthsLeft).toBeCloseTo(24, 1);
    expect(p.perMonth).toBeCloseTo(100_000 / 24, -1);
    expect(p.pace).toBeNull();
    expect(p.arrival).toBeNull();
  });

  it("usa o ponto de 12 meses atrás para o ritmo e prevê a chegada", () => {
    const history = [
      { date: "2025-09-30", value: 170_000 },
      { date: "2025-10-24", value: 176_000 },
      { date: "2026-06-30", value: 190_000 },
    ];
    const p = progress(now, { value: 224_000, date: "2027-12-31" }, history);
    // 2025-10-24 é o mais perto de 12 meses atrás: +24 mil em ~12 meses.
    expect(p.pace).toBeCloseTo(2_000, -1);
    expect(p.arrival?.slice(0, 7)).toBe("2027-10");
  });

  it("com pouco histórico usa o ponto mais antigo; menos de 2 meses não dá ritmo", () => {
    expect(progress(now, { value: 300_000, date: "2030-01-01" }, [{ date: "2026-10-01", value: 199_000 }]).pace).toBeNull();
    const p = progress(now, { value: 300_000, date: "2030-01-01" }, [{ date: "2026-04-25", value: 188_000 }]);
    expect(p.paceMonths).toBeCloseTo(6, 0);
    expect(p.pace).toBeCloseTo(2_000, -1);
  });

  it("meta já alcançada", () => {
    const p = progress(now, { value: 150_000, date: "2027-01-01" }, []);
    expect(p.missing).toBe(0);
    expect(p.done).toBeGreaterThan(1);
    expect(p.arrival).toBe("2026-10-25");
    expect(p.perMonth).toBeNull();
  });
});
