import { describe, expect, it } from "vitest";
import type { GroupMonth, IncomeRow, TaxMonth, TaxView } from "./api";
import { bens, countryOf, exclusivos, isentos, rendaVariavel } from "./telasIr";

const g = (
  group: GroupMonth["group"],
  result: number,
  before: number,
  used: number,
  after: number,
  base: number,
  rate: number,
  tax: number,
): GroupMonth => ({
  group,
  result: String(result),
  loss_before: String(before),
  loss_used: String(used),
  loss_after: String(after),
  base: String(base),
  rate: String(rate),
  tax: String(tax),
});

const month = (
  month: string,
  groups: GroupMonth[],
  irrf: { total: number; dt?: number; fii?: number },
  sales: TaxMonth["sales"],
  paid: number | null = null,
): TaxMonth => ({
  month,
  stock_sales: "0",
  exempt: false,
  exempt_profit: "0",
  groups,
  gross_tax: String(groups.reduce((s, x) => s + Number(x.tax), 0)),
  irrf_month: String(irrf.total),
  irrf_dt: String(irrf.dt ?? 0),
  irrf_fii: String(irrf.fii ?? 0),
  irrf_before: "0",
  irrf_used: "0",
  irrf_after: "0",
  small_before: "0",
  due: "0",
  darf: "0",
  small_after: "0",
  due_date: month + "-28",
  sales,
  status: paid != null ? "PAGO" : "SEM_DARF",
  paid: paid != null ? { month, paid_on: month + "-28", amount: String(paid) } : null,
});

const sale = (group: "COMUM" | "DAY_TRADE" | "FII") =>
  ({ group, day_trade: group === "DAY_TRADE" }) as TaxMonth["sales"][number];

describe("telas do IR: renda variável", () => {
  const months = [
    // Dezembro do ano anterior: prejuízo de 1.000 em operações comuns.
    month("2025-12", [g("COMUM", -1000, 0, 0, 1000, 0, 0.15, 0)], { total: 0 }, [sale("COMUM")]),
    // Março: prejuízo de 500 → 1.500 a compensar.
    month("2026-03", [g("COMUM", -500, 1000, 0, 1500, 0, 0.15, 0)], { total: 1.65 }, [sale("COMUM")]),
    // Maio: lucro 3.500 comum, abate 1.500; day trade 200; FII 1.000.
    month(
      "2026-05",
      [
        g("COMUM", 3500, 1500, 1500, 0, 2000, 0.15, 300),
        g("DAY_TRADE", 200, 0, 0, 0, 200, 0.2, 40),
        g("FII", 1000, 0, 0, 0, 1000, 0.2, 200),
      ],
      { total: 2 + 3.5 + 1.2, dt: 2, fii: 1.2 },
      [sale("COMUM"), sale("DAY_TRADE"), sale("FII")],
      // DARF único pago: (300 + 40 − 2 − 1,65 − 3,5) + (200 − 1,2).
      332.85 + 198.8,
    ),
  ];
  const { comuns, fii } = rendaVariavel(months, 2026);

  it("leva o prejuízo de dezembro para janeiro e pelos meses sem venda", () => {
    expect(comuns[0].comum.lossBefore).toBe(1000);
    expect(comuns[0].hasSales).toBe(false);
    expect(comuns[1].comum.lossAfter).toBe(1000);
    expect(comuns[2].comum.lossAfter).toBe(1500);
    expect(comuns[3].comum.lossBefore).toBe(1500);
  });

  it("separa o IR na fonte de day trade e o dedo-duro e abate do imposto", () => {
    const mar = comuns[2];
    expect(mar.irrfLeiMonth).toBe(1.65);
    expect(mar.irrfLeiComp).toBe(1.65);
    const mai = comuns[4];
    expect(mai.totalTax).toBe(340);
    expect(mai.irrfDtMonth).toBe(2);
    expect(mai.irrfLeiBefore).toBe(1.65);
    expect(mai.irrfLeiMonth).toBe(3.5);
    expect(mai.toPay).toBe(332.85);
    expect(mai.irrfLeiComp).toBe(0);
    expect(mai.paid).toBe(332.85);
  });

  it("FII tem tela e IR na fonte próprios", () => {
    const mai = fii[4];
    expect(mai.hasSales).toBe(true);
    expect(mai.tax).toBe(200);
    expect(mai.irrfMonth).toBe(1.2);
    expect(mai.toPay).toBe(198.8);
    expect(mai.paid).toBe(198.8);
  });

  it("os valores das duas telas somam o DARF do app", () => {
    const m = months[2];
    const total = Number(m.gross_tax) - Number(m.irrf_month);
    expect(comuns[4].toPay + fii[4].toPay).toBeCloseTo(total - 1.65, 2);
  });
});

const inc = (asset: string, kind: string, pay: string, net: number): IncomeRow =>
  ({
    income: { id: asset + pay, account: "X", asset, kind, pay_date: pay, ex_date: null, gross: String(net), withheld: "0", currency: "BRL", note: null },
    net: String(net),
    gross_brl: String(net),
    withheld_brl: "0",
    net_brl: String(net),
    fx: null,
    quantity: null,
    per_share: null,
  }) as IncomeRow;

describe("telas do IR: rendimentos", () => {
  const rows = [
    inc("TEST3", "DIVIDENDO", "2026-03-10", 100),
    inc("TEST3", "DIVIDENDO", "2026-09-10", 50.5),
    inc("FUND11", "RENDIMENTO", "2026-02-15", 80),
    inc("TEST4", "JCP", "2026-04-01", 85),
    inc("TEST3", "DIVIDENDO", "2025-12-10", 999),
  ];
  it("um item por fonte pagadora, só do ano", () => {
    const l = isentos(rows, [], [], { TEST3: "Empresa Teste" }, 2026);
    expect(l.map((x) => x.id)).toEqual(["09", "99"]);
    expect(l[0].payers).toEqual([{ asset: "TEST3", name: "Empresa Teste", value: 150.5 }]);
    expect(l[1].confirm).toBe(true);
    const e = exclusivos(rows, {}, 2026);
    expect(e[0]).toMatchObject({ id: "10", total: 85 });
  });
});

describe("telas do IR: bens e direitos", () => {
  it("mostra 31/12 do ano anterior e do ano, e o lucro no exterior", () => {
    const view = {
      annual: {
        positions: {
          "2025": [
            { asset: "TEST3", quantity: "10", cost_brl: "100", cost_usd: null },
            { asset: "ETFX", quantity: "2", cost_brl: "1000", cost_usd: "200" },
          ],
          "2026": [{ asset: "TEST3", quantity: "20", cost_brl: "220", cost_usd: null }],
        },
        foreign: [{ year: 2026, sales: [{ asset: "ETFX", result_brl: "150" }] }],
        warnings: [],
      },
      savings_year_end: [],
    } as unknown as TaxView;
    const b = bens(view, { TEST3: "ACAO", ETFX: "ETF" }, {}, 2026);
    expect(b[0]).toMatchObject({ key: "ETFX", before: 1000, after: 0, country: "249 - Estados Unidos", profit: 150, group: null });
    expect(b[1]).toMatchObject({ key: "TEST3", before: 100, after: 220, group: "03", code: "01", ticker: "TEST3", profit: null });
  });
  it("deixa o FGTS e o cartão benefício de fora", () => {
    const view = {
      annual: { positions: {}, foreign: [], warnings: [] },
      savings_year_end: [
        { year: 2026, name: "FGTS", account: "", kind: "FGTS", value: "500" },
        { year: 2026, name: "Benefício", account: "Banco A", kind: "Cartão benefício", value: "80" },
        { year: 2026, name: "Conta corrente", account: "Banco A", kind: "Conta corrente", value: "300" },
      ],
    } as unknown as TaxView;
    const b = bens(view, {}, {}, 2026);
    expect(b.map((x) => x.key)).toEqual(["Conta corrente|Conta corrente|Banco A"]);
  });
});

describe("telas do IR: país do bem no exterior", () => {
  it("ETF de Londres é da Irlanda; o resto, dos EUA", () => {
    expect(countryOf("CSPX.L")).toBe("375 - Irlanda");
    expect(countryOf("VOO")).toBe("249 - Estados Unidos");
  });
});
