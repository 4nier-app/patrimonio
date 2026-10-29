// Notas SINTÉTICAS: nomes, números e valores inventados, imitando só o
// layout público de cada tipo de nota. Notas reais nunca entram no projeto.

import { describe, expect, it } from "vitest";
import { readNotes } from "./index";
import { buildLines } from "./lines";
import { allocate, brNumber, grossCents, usNumber } from "./numbers";

// Taxas sobre R$ 1.000,00 com a tabela B3: 0,22 + 0,05 + 0,02 = 0,29.
const sinacorComRS = [
  [
    "Corretora Exemplo S.A. - Corretora de Títulos e Valores Mobiliários  Data Pregão  Folha",
    "CNPJ: 00.000.000/0001-00 | www.exemplo.com.br  02/03/2026  1 / 2",
    "Rua Fictícia, 1  Número da nota",
    "00000-000 | Cidade | UF | BR  123456",
    "Mercado  C/V Tipo de Mercado  Especificação do Título  Observação  Quantidade  Preço/Ajuste  Valor/Ajuste D/C",
    "BOVESPA  C  VISTA  TEST11 CI ER  @  50  R$ 10,00  R$ 500,00 D",
    "BOVESPA  C FRACIONARIO  ABCD3F ON NM  @#  25  R$ 20,00  R$ 500,00 D",
  ],
  [
    "Corretora Exemplo S.A. - Corretora de Títulos e Valores Mobiliários  Data Pregão  Folha",
    "CNPJ: 00.000.000/0001-00 | www.exemplo.com.br  02/03/2026  2 / 2",
    "Rua Fictícia, 1  Número da nota",
    "00000-000 | Cidade | UF | BR  123456",
    "Resumo dos Negócios  Resumo Financeiro",
    "Vendas à vista  R$ 0,00  Valor Líquido das Operações  -R$ 1.000,00",
    "Compras à vista  R$ 1.000,00  Taxa de Liquidação/CCP  -R$ 0,22",
    "Opções - Compras  R$ 0,00  Taxa de Registro  R$ 0,00",
    "Valor das Operações  R$ 1.000,00",
    "Especificações Diversas  Emolumentos  -R$ 0,05",
    "IRRF Day Trade: Base R$ 0,00  Total Bolsa  -R$ 0,05",
    "Taxa de Transferência de Ativos  -R$ 0,02",
    "contra parte  Corretagem  R$ 0,00",
    "I.R.R.F. s/ operações. Base R$ 0,00  R$ 0,00",
    "Líquido para 04/03/2026  -R$ 1.000,29",
  ],
];

// Mesmo padrão, sem "R$", com D/C no fim e nome de pregão no lugar do código.
const sinacorSemRS = [
  [
    "NOTA DE CORRETAGEM",
    "Nr. Nota  Folha  Data pregão",
    "777  1  10/04/2026",
    "CORRETORA EXEMPLO S.A - CTVM",
    "Negocios realizados",
    "B3 RV LISTADO  C  VISTA  EMPRESA TESTE PN N2  @  100  30,00  3.000,00  D",
    "B3 RV LISTADO  V  VISTA  OUTRA TESTE ON NM  200  10,00  2.000,00  C",
    "Resumo dos Negócios  Resumo Financeiro  D/C",
    "Vendas à vista  2.000,00  Valor líquido das operações  1.000,00 D",
    "Compras à vista  3.000,00  Taxa de liquidação/CCP  1,12 D",
    "Valor das operações  5.000,00",
    "Emolumentos  0,25 D",
    "Taxa de Transferência de Ativos  0,13 D",
    "Clearing  0,00 D",
    "Execução  0,00 D",
    "I.R.R.F. s/ operações, base 2.000,00  0,10",
    "Líquido para 14/04/2026  1.001,60 D",
  ],
];

// Repetições de texto como em notas com camada de acessibilidade.
const sinacorRepetido = [
  [
    "Número da nota 55Número da nota 555  Folha 1Folha 1  Data pregão 05/05/2026Data pregão 05/05/2026",
    "Nº da Nota  Folha  Data do pregão",
    "555  1  05/05/2026",
    "Negócios realizados",
    "B3 RV LISTADO  CompraCompraC  VISTA  EMPRESA TESTE ON NM  @  100  R$ 10,00  R$ 1.000,00  DébitoDébitoD",
    "Valor das operações R$ 1.000,00Valor das operações R$ 1.000,00",
    "Taxa de liquidação/CCP R$ 0,22 DébitoTaxa de liquidação/CCP R$ 0,22 Débito",
    "Emolumentos R$ 0,05 DébitoEmolumentos R$ 0,05 Débito",
    "Taxa de transferência de ativos R$ 0,02 DébitoTaxa de transferência de ativos R$ 0,02 Débito",
    "Líquido para 07/05/2026 R$ 1.000,29 DébitoLíquido para 07/05/2026 R$ 1.000,29 Débito",
  ],
];

// Layout da XP: código depois do nome de pregão e "Taxa de Transf. de Ativos".
const sinacorXp = [
  [
    "NOTA DE NEGOCIAÇÃO",
    "Nr. nota  Folha  Data pregão",
    "888  1  06/07/2026",
    "XP INVESTIMENTOS CORRETORA DE CÂMBIO, TÍTULOS E VALORES MOBILIÁRIOS S.A.",
    "Negócios realizados",
    "1-BOVESPA  C VISTA  FII TESTE REN  TEST11  CI  @  200  5,00  1.000,00 D",
    "Resumo dos Negócios  Resumo Financeiro",
    "Compras à vista  1.000,00 Valor líquido das operações  1.000,00 D",
    "Valor das operações  1.000,00  Taxa de liquidação  0,22 D",
    "Emolumentos  0,05 D",
    "Taxa de Transf. de Ativos  0,02 D",
    "Líquido para 08/07/2026  1.000,29 D",
  ],
];

describe("números", () => {
  it("lê os formatos brasileiro e americano", () => {
    expect(brNumber("R$ 1.234,56")).toBe("1234.56");
    expect(brNumber("-R$ 0,22")).toBe("-0.22");
    expect(brNumber("2700")).toBe("2700");
    expect(usNumber("$1,723.79")).toBe("1723.79");
    expect(usNumber("-0.00000774")).toBe("-0.00000774");
  });
  it("multiplica e rateia em centavos", () => {
    expect(grossCents("0.92681008", "46.7988")).toBe(4337n);
    expect(allocate(29n, [50000n, 50000n])).toEqual([15n, 14n]);
    expect(allocate(7n, [100n, 300n, 600n])).toEqual([0n, 2n, 5n]);
  });
});

describe("linhas do PDF", () => {
  it("separa colunas por dois espaços e junta pedaços da mesma palavra", () => {
    const lines = buildLines([
      { str: "Taxa", x: 10, y: 700, width: 20, height: 8 },
      { str: "de", x: 32, y: 700, width: 10, height: 8 },
      { str: "R$ 0,22", x: 200, y: 700.5, width: 30, height: 8 },
      { str: "Valor", x: 10, y: 680, width: 22, height: 8 },
    ]);
    expect(lines).toEqual(["Taxa de  R$ 0,22", "Valor"]);
  });
});

describe("nota SINACOR", () => {
  it("lê nota de duas folhas com código e rateia as taxas", () => {
    const [n] = readNotes(sinacorComRS);
    expect(n.reader).toBe("sinacor");
    expect(n.number).toBe("123456");
    expect(n.date).toBe("2026-03-02");
    expect(n.check.ok).toBe(true);
    expect(n.totalFees).toBe("0.29");
    expect(n.lines.map((l) => [l.ticker, l.market, l.quantity, l.price, l.fees])).toEqual([
      ["TEST11", "VISTA", "50", "10.00", "0.15"],
      ["ABCD3", "FRACIONARIO", "25", "20.00", "0.14"],
    ]);
  });

  it("lê nota sem R$, com venda, nome de pregão e IRRF", () => {
    const [n] = readNotes(sinacorSemRS);
    expect(n.number).toBe("777");
    expect(n.check).toEqual(expect.objectContaining({ ok: true }));
    expect(n.lines[0]).toEqual(
      expect.objectContaining({ side: "COMPRA", ticker: null, lookup: { name: "EMPRESA TESTE", cls: "PN" } }),
    );
    expect(n.lines[1]).toEqual(expect.objectContaining({ side: "VENDA", quantity: "200", value: "2000.00" }));
    expect(n.totalFees).toBe("1.50");
    expect(n.warnings.join(" ")).toContain("IRRF");
  });

  it("aceita nota com IRRF fora do líquido", () => {
    const pages = sinacorSemRS.map((p) => p.map((l) => (l.startsWith("Líquido para") ? "Líquido para 14/04/2026  1.001,50 D" : l)));
    const [n] = readNotes(pages);
    expect(n.check.ok).toBe(true);
    expect(n.warnings.join(" ")).toContain("não foi descontado do líquido");
  });

  it("aguenta texto repetido e ignora a repetição", () => {
    const [n] = readNotes(sinacorRepetido);
    expect(n.number).toBe("555");
    expect(n.check.ok).toBe(true);
    expect(n.lines).toHaveLength(1);
    expect(n.lines[0].fees).toBe("0.29");
  });

  it("lê nota da XP com o código depois do nome", () => {
    const [n] = readNotes(sinacorXp);
    expect(n.broker).toBe("XP");
    expect(n.lines[0].ticker).toBe("TEST11");
    expect(n.totalFees).toBe("0.29");
    expect(n.check.ok).toBe(true);
  });

  it("acusa quando o líquido não bate", () => {
    const pages = sinacorRepetido.map((p) => p.map((l) => l.replace(/1\.000,29/g, "1.000,30")));
    const [n] = readNotes(pages);
    expect(n.check.ok).toBe(false);
    expect(n.check.text).toContain("líquido");
  });

  it("a mesma nota em outro arquivo tem a mesma identidade", () => {
    const a = readNotes(sinacorComRS)[0];
    const b = readNotes([[...sinacorComRS[0], ...sinacorComRS[1].slice(4)]])[0];
    expect(a.identity).toBe(b.identity);
  });
});

describe("Nomad", () => {
  const drivewealth = [
    [
      "NOMAD",
      "Transaction  000 Fictitious Road",
      "Confirmation",
      "Account Number:TEST-000  Account Name: CLIENTE FICTICIO",
      "Confirmation Date : 4/3/2025",
      "Symbol  Security  A/CType  Action  Execution Time  Quantity  Price  Trade Date  Settle Date  Capacity",
      "TSTA  TEST FUND ETF  M  Buy  9:55:55 AM  3  50.00  4/3/2025  4/4/2025  Agency",
      "Principal Amount  $150.00",
      "Commission  $0.00",
      "Transaction Fee  $0.00",
      "Other Fees / Credits  $0.00",
      "Net Amount  $150.00",
      "TSTA  TEST FUND ETF  M  Buy  9:55:55 AM  0.5  50.00  4/3/2025  4/4/2025  Riskless Principal",
      "Principal Amount  $25.00",
      "Commission  $0.00",
      "Transaction Fee  $0.00",
      "Other Fees / Credits  $0.00",
      "Net Amount  $25.00",
      "TSTB  OTHER TEST ETF  M  Sell  1:18:43 PM  -0.00000774  600.00  4/3/2025  4/4/2025  Principal",
      "Principal Amount  $0.00",
      "Commission  $0.00",
      "Transaction Fee  $0.00",
      "Other Fees / Credits  $0.00",
      "Net Amount  $0.00",
      "Clearing and execution services provided by DriveWealth, LLC  Page 1 of 1",
    ],
  ];

  it("antigo (DriveWealth): junta inteira e fração e desmarca resíduo", () => {
    const [n] = readNotes(drivewealth);
    expect(n.reader).toBe("nomad-drivewealth");
    expect(n.currency).toBe("USD");
    expect(n.date).toBe("2025-04-03");
    expect(n.check.ok).toBe(true);
    expect(n.lines[0]).toEqual(expect.objectContaining({ ticker: "TSTA", quantity: "3.5", price: "50.00", value: "175.00" }));
    expect(n.lines[1].skip).toContain("custodiante");
  });

  it("atual (Apex): compras e vendas por seção", () => {
    const [n] = readNotes([
      [
        "Fictitious Investment Services Inc.",
        "Trade Confirmation",
        "Process Date:  2026-09-30",
        "You Bought",
        "Trade Date  Settle Date  Symbol  Quantity  Price  Gross Amount  Commission  Transaction Fee  Add'l Fees  Net Amount",
        "2026-09-30  2026-10-01  TSTA  1.5  40.00  60.00  0.00  0.00  0.00  60.00",
        "DESC: TEST FUND ETF  Trade#: AAA111  CAP: AGENCY",
        "You Sold",
        "2026-09-30  2026-10-01  TSTB  2  10.00  20.00  0.00  0.01  0.00  19.99",
        "DESC: OTHER TEST ETF  Trade#: BBB222  CAP: AGENCY",
      ],
    ]);
    expect(n.reader).toBe("nomad-apex");
    expect(n.check.ok).toBe(true);
    expect(n.lines.map((l) => [l.side, l.ticker, l.quantity, l.fees])).toEqual([
      ["COMPRA", "TSTA", "1.5", "0.00"],
      ["VENDA", "TSTB", "2", "0.01"],
    ]);
  });

  it("recusa arquivo que não é nota", () => {
    expect(() => readNotes([["Extrato mensal", "Saldo  R$ 10,00"]])).toThrow(/nota de corretagem/);
  });
});
