import { describe, expect, it } from "vitest";
import type { IncomeKind, IncomeRow } from "./api";
import { byAsset, byMonth, byYear, defaultKind, foreignTaxDue, last12, projection, taxGroups, totals, years } from "./incomes";

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

describe("projeção dos próximos 12 meses", () => {
  const withQty = (r: IncomeRow, q: number): IncomeRow => ({ ...r, quantity: String(q) });
  const base = [
    withQty(row("AAAA3", "DIVIDENDO", "2026-03-10", 100, 0), 100),
    withQty(row("AAAA3", "DIVIDENDO", "2025-11-10", 50, 0), 100),
    withQty(row("BBBB11", "RENDIMENTO", "2026-09-15", 20, 0), 10),
    row("CONTA", "JUROS", "2026-10-02", 5, 0),
    withQty(row("VELHO3", "DIVIDENDO", "2025-01-10", 999, 0), 1),
  ];

  it("repete no mesmo mês, ajustado pelas cotas de hoje", () => {
    const p = projection(base, "2026-10-25", { AAAA3: 150, BBBB11: 0 });
    expect(p.months[0]).toBe("2026-11");
    expect(p.months[11]).toBe("2027-10");
    expect(p.values[0]).toBe(75); // nov: 50 × 1,5
    expect(p.values[4]).toBe(150); // mar: 100 × 1,5
    expect(p.values[10]).toBe(0); // set: BBBB11 vendido
    expect(p.values[11]).toBe(5); // out: juros sem posição, como foi
    expect(p.total).toBe(230);
    expect(p.received).toBe(175); // VELHO3 está fora dos 12 meses
    expect(p.assets.map((a) => [a.asset, a.projected, a.factor, a.held])).toEqual([
      ["AAAA3", 225, 1.5, 150],
      ["CONTA", 5, null, null],
      ["BBBB11", 0, 0, 0],
    ]);
  });

  it("deixa de fora os sem PTAX", () => {
    const p = projection([row("TSTY", "DIVIDENDO", "2026-02-02", 10, 3, "USD", 0)], "2026-10-25", {});
    expect(p.missingFx).toBe(1);
    expect(p.total).toBe(0);
  });
});
