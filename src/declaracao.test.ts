import { describe, expect, it } from "vitest";
import type { IncomeKind, IncomeRow, SuggestedIncome, TaxMonth } from "./api";
import { bensCode, discriminacao, incomeLines, pendencias } from "./declaracao";

const row = (
  asset: string,
  kind: IncomeKind,
  pay: string,
  gross: number,
  withheld: number,
  currency: "BRL" | "USD" = "BRL",
  rate: number | null = 1,
): IncomeRow => ({
  income: {
    id: asset + pay,
    account: "X",
    asset,
    kind,
    pay_date: pay,
    ex_date: null,
    gross: String(gross),
    withheld: String(withheld),
    currency,
    note: null,
  },
  net: String(gross - withheld),
  gross_brl: rate ? String(gross * rate) : null,
  withheld_brl: rate ? String(withheld * rate) : null,
  net_brl: rate ? String((gross - withheld) * rate) : null,
  fx: null,
  quantity: null,
  per_share: null,
});

describe("resumo para o contador", () => {
  it("dá grupo e código só quando o tipo é claro", () => {
    expect(bensCode("TEST3", "ACAO")).toMatchObject({
      group: "03",
      code: "01",
    });
    expect(bensCode("FUND11", "FII")).toMatchObject({
      group: "07",
      code: "03",
    });
    expect(bensCode("BTC", "CRIPTO")).toMatchObject({
      group: "08",
      code: "01",
    });
    expect(bensCode("ETH", "CRIPTO")).toMatchObject({
      group: "08",
      code: "02",
    });
    expect(bensCode("USDC", "CRIPTO")).toMatchObject({
      group: "08",
      code: "03",
    });
    expect(bensCode("TEST11", "ETF")).toBeNull();
    expect(bensCode("XYZ", undefined)).toBeNull();
  });

  it("escreve a discriminação com o custo médio", () => {
    expect(
      discriminacao(
        { asset: "TEST3", quantity: "100", cost_brl: "1000", cost_usd: null },
        "ACAO",
        "Teste SA",
      ),
    ).toBe("100 ações TEST3 (Teste SA), custo médio R$ 10,00");
    expect(
      discriminacao(
        { asset: "TSTX", quantity: "4", cost_brl: "2000", cost_usd: "400" },
        "ETF",
        undefined,
      ),
    ).toBe("4 cotas TSTX, custo médio US$ 100,00, total US$ 400,00");
  });

  it("separa os proventos por linha da declaração", () => {
    const rows = [
      row("TEST3", "DIVIDENDO", "2026-03-20", 50, 0),
      row("TEST3", "JCP", "2026-03-10", 100, 15),
      row("FUND11", "RENDIMENTO", "2026-04-15", 30, 0),
      row("FUND11", "RENDIMENTO", "2025-12-15", 30, 0),
      row("TSTX", "DIVIDENDO", "2026-02-01", 10, 3, "USD", 5),
      row("TSTY", "DIVIDENDO", "2026-02-02", 10, 3, "USD", null),
    ];
    const l = incomeLines(rows, 2026);
    expect(l.isentos.map((x) => [x.id, x.net])).toEqual([
      ["dividendos", 50],
      ["fii", 30],
    ]);
    expect(l.exclusivos).toMatchObject([
      { id: "jcp", net: 85, gross: 100, withheld: 15 },
    ]);
    expect(l.exterior).toMatchObject({ gross: 50, withheld: 15 });
    expect(l.missingFx).toBe(1);
  });

  it("lista as pendências do ano", () => {
    const sg = (asset: string, ex: string) =>
      ({ asset, ex_date: ex }) as SuggestedIncome;
    const m = (month: string, status: TaxMonth["status"], darf: string) =>
      ({ month, status, darf }) as TaxMonth;
    const p = pendencias(
      2026,
      [
        m("2026-05", "PENDENTE", "120"),
        m("2025-05", "ATRASADO", "50"),
        m("2026-06", "PAGO", "10"),
      ],
      [
        sg("TEST3", "2026-03-01"),
        sg("FUND11", "2026-04-01"),
        sg("OLD3", "2025-01-01"),
      ],
      0,
    );
    expect(p.map((x) => x.text)).toEqual([
      "2 proventos de 2026 que faltam lançar (FUND11, TEST3)",
      "DARF de maio em aberto: R$ 120,00",
    ]);
  });
});
