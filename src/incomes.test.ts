import { describe, expect, it } from "vitest";
import type { IncomeKind, IncomeRow } from "./api";
import { byAsset, byMonth, byYear, defaultKind, foreignTaxDue, last12, taxGroups, totals, years } from "./incomes";

const row = (asset: string, kind: IncomeKind, pay: string, gross: number, withheld: number, currency: "BRL" | "USD" = "BRL", rate = 1): IncomeRow => ({
  income: { id: asset + pay, account: "X", asset, kind, pay_date: pay, ex_date: null, gross: String(gross), withheld: String(withheld), currency, note: null },
  net: String(gross - withheld),
  gross_brl: rate ? String(gross * rate) : null,
  withheld_brl: rate ? String(withheld * rate) : null,
  net_brl: rate ? String((gross - withheld) * rate) : null,
  fx: null,
  quantity: null,
  per_share: null,
});

const rows = [
  row("TEST3", "JCP", "2026-03-10", 100, 15),
  row("TEST3", "DIVIDENDO", "2026-03-20", 50, 0),
  row("FUND11", "RENDIMENTO", "2025-12-15", 30, 0),
  row("TSTX", "DIVIDENDO", "2026-02-01", 10, 3, "USD", 5),
  row("TSTY", "DIVIDENDO", "2026-02-02", 10, 3, "USD", 0),
];

describe("proventos", () => {
  it("soma em reais e separa os sem PTAX", () => {
    expect(totals(rows)).toEqual({ gross: 230, withheld: 30, net: 200, missingFx: 1 });
    expect(years(rows)).toEqual([2026, 2025]);
  });

  it("agrupa por mês, ano e ativo", () => {
    const m = byMonth(rows, 2026);
    expect(m.JCP[2]).toBe(85);
    expect(m.DIVIDENDO[2]).toBe(50);
    expect(m.DIVIDENDO[1]).toBe(35);
    const y = byYear(rows);
    expect(y.years).toEqual([2025, 2026]);
    expect(y.values.RENDIMENTO).toEqual([30, 0]);
    expect(byAsset(rows).map((a) => [a.asset, a.net, a.count])).toEqual([
      ["TEST3", 135, 2],
      ["TSTX", 35, 1],
      ["FUND11", 30, 1],
      ["TSTY", 0, 1],
    ]);
    expect(last12(rows, "2026-12-15").map((r) => r.income.asset)).toEqual(["TEST3", "TEST3", "TSTX", "TSTY"]);
    expect(defaultKind("FII")).toBe("RENDIMENTO");
    expect(defaultKind("ACAO")).toBe("DIVIDENDO");
  });

  it("resume para a declaração", () => {
    const g = taxGroups(rows);
    expect(g.map((x) => [x.id, x.gross, x.withheld, x.count])).toEqual([
      ["isento", 80, 0, 2],
      ["exclusivo", 100, 15, 1],
      ["exterior", 50, 15, 2],
    ]);
    expect(foreignTaxDue(g[2])).toBe(0);
    expect(foreignTaxDue({ id: "exterior", gross: 100, withheld: 10, net: 90, count: 1 })).toBe(5);
  });
});
