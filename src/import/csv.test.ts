// Planilhas SINTÉTICAS: contas, ativos e valores inventados.

import { describe, expect, it } from "vitest";
import { CSV_TEMPLATE, parseCsv } from "./csv";

describe("planilha CSV", () => {
  it("lê o modelo", () => {
    const { rows, errors } = parseCsv(CSV_TEMPLATE);
    expect(errors).toEqual([]);
    expect(rows).toHaveLength(3);
    expect(rows[0]).toMatchObject({ date: "2026-03-02", account: "Corretora Exemplo", asset: "ABCD3", side: "COMPRA", quantity: "100", price: "10.50", fees: null, currency: "BRL" });
    expect(rows[1]).toMatchObject({ side: "VENDA", fees: "0.35" });
    expect(rows[2]).toMatchObject({ quantity: "0.5", price: "480.1234", currency: "USD", fees: "0" });
  });

  it("aceita vírgula como separador, ponto decimal e datas ISO", () => {
    const { rows, errors } = parseCsv('Data,Corretora,Ticker,Tipo,Qtd,Preço\n2026-01-05,Teste,"XYZW11",C,"1,000",95.84\n');
    expect(errors).toEqual([]);
    expect(rows[0]).toMatchObject({ date: "2026-01-05", asset: "XYZW11", quantity: "1000", price: "95.84" });
  });

  it("milhar com ponto quando o separador é ponto e vírgula", () => {
    const { rows } = parseCsv("data;conta;ativo;operacao;quantidade;preco\n05/01/2026;X;ABCD3;V;1.500;1.234,56\n");
    expect(rows[0]).toMatchObject({ quantity: "1500", price: "1234.56" });
  });

  it("aponta a linha com erro e segue nas outras", () => {
    const { rows, errors } = parseCsv("data;conta;ativo;operacao;quantidade;preco\n31/13/2026;X;A;C;1;1\n05/01/2026;X;ABCD3;Troca;1;1\n05/01/2026;X;ABCD3;C;2;3\n");
    expect(rows).toHaveLength(1);
    expect(errors.map((e) => e.line)).toEqual([2, 3]);
  });

  it("cabeçalho sem as colunas obrigatórias", () => {
    const { errors } = parseCsv("data;ativo\n");
    expect(errors[0].message).toContain("conta");
  });
});
