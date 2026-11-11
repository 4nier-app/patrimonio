import { describe, expect, it } from "vitest";
import type { EvolutionGroup, EvolutionRow, EvolutionView } from "./api";
import { evolutionSheet } from "./screens/Evolution";
import { stamp } from "./planilha";

const row = (name: string, values: (string | null)[], extra: Partial<EvolutionRow> = {}): EvolutionRow => ({
  asset: null, savings_id: null, name, detail: null, values, missing: values.map(() => false), outside: values.map(() => false),
  gain: null, since: null, notes: [], value_label: null, lots: [], usd: [], ...extra,
});
const group = (id: string, label: string, rows: EvolutionRow[], total: (string | null)[]): EvolutionGroup => ({
  id, label, investment: id !== "bens" && id !== "fgts", rows, total, gain: null, since: null,
});

const view: EvolutionView = {
  years: [2026],
  year: 2026,
  columns: [
    { date: "2025-12-31", kind: "BASE" },
    { date: "2026-10-31", kind: "MONTH" },
    { date: "2026-11-10", kind: "YESTERDAY" },
    { date: "2026-11-11", kind: "TODAY" },
  ],
  groups: [
    group("b3", "Ações e ETFs na B3", [row("ABCD3", ["100", "110", "120", "121"], { gain: { value: "21", base: "100" } })], ["100", "110", "120", "121"]),
    group("bens", "Bens (imóveis e veículos)", [row("Carro", ["50", "50", "50", "50"])], ["50", "50", "50", "50"]),
  ],
  total: ["150", "160", "170", "171"],
  investments: ["100", "110", "120", "121"],
  new_money: [null, "0", "0", "0"],
  total_gain: { value: "21", base: "150" },
  investments_gain: { value: "21", base: "100" },
  investments_since: null,
  investments_incomes: "0",
  incomplete: false,
} as EvolutionView;

describe("planilha da Evolução", () => {
  const sheet = evolutionSheet(view, new Date(2026, 10, 11, 15, 45));

  it("tem as colunas da tela, com Hoje antes de Ontem", () => {
    expect(sheet.rows[0].cells.slice(1, 5)).toEqual(["31/12/25 início", "OUT 31/10", "HOJE 11/11", "ONTEM 10/11"]);
    expect(sheet.rows[0].cells).toHaveLength(9);
  });

  it("traz os totais da tela: investimentos, rendimento, sem bens, variação e total geral", () => {
    const labels = sheet.rows.map((r) => r.cells[0]);
    expect(labels.slice(-5)).toEqual(["Total investimentos", "   Rendimento", "Total sem bens", "   Variação (R$)", "Total geral (com bens)"]);
    const semBens = sheet.rows.find((r) => r.cells[0] === "Total sem bens")!;
    expect(semBens.cells.slice(1, 5)).toEqual([{ n: 100, f: "money" }, { n: 110, f: "money" }, { n: 121, f: "money" }, { n: 120, f: "money" }]);
    // No período: 121 − 100, e a %.
    expect(semBens.cells[5]).toEqual({ n: 21, f: "money" });
    expect(semBens.cells[6]).toEqual({ n: 0.21, f: "pct" });
  });

  it("diz quando foi exportada", () => {
    expect(sheet.note).toContain("Exportado em 11/11/2026 15:45");
    expect(stamp(new Date(2026, 0, 2, 3, 4))).toBe("02/01/2026 03:04");
  });
});
