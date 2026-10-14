import { describe, expect, it } from "vitest";
import { cashMask, date, maskDecimal, money, parseDecimalInput, passwordStrength, percent, shortHash, toCashMask } from "./format";

describe("shortHash", () => {
  it("encurta hashes longos", () => {
    expect(shortHash("abcdef0123456789", 6)).toBe("abcdef…");
  });
  it("mantém textos curtos", () => {
    expect(shortHash("abc")).toBe("abc");
  });
});

describe("passwordStrength", () => {
  it("vai de fraca a forte", () => {
    expect(passwordStrength("abc")).toBe(0);
    expect(passwordStrength("abcdefgh")).toBe(1);
    expect(passwordStrength("Abcdefgh1234!")).toBe(4);
  });
});

describe("parseDecimalInput", () => {
  it("aceita o padrão brasileiro", () => {
    expect(parseDecimalInput("1.234,56")).toBe("1234.56");
    expect(parseDecimalInput("12,5")).toBe("12.5");
    expect(parseDecimalInput("R$ 10,00")).toBe("10.00");
    expect(parseDecimalInput("1.500")).toBe("1500");
    expect(parseDecimalInput("12.5")).toBe("12.5");
    expect(parseDecimalInput("100")).toBe("100");
  });
  it("recusa texto inválido", () => {
    expect(parseDecimalInput("")).toBeNull();
    expect(parseDecimalInput("abc")).toBeNull();
    expect(parseDecimalInput("-5")).toBeNull();
    expect(parseDecimalInput("1,2,3")).toBeNull();
  });
});

describe("exibição", () => {
  it("formata datas e percentuais", () => {
    expect(date("2026-03-02")).toBe("02/03/2026");
    expect(percent("0.0523")).toBe("5,23%");
    expect(percent(null)).toBe("—");
  });
});

describe("maskDecimal", () => {
  it("formata dinheiro", () => {
    expect(maskDecimal("1234,5", "money")).toBe("R$ 1.234,50");
    expect(maskDecimal("10", "money")).toBe("R$ 10,00");
    expect(maskDecimal("0,1234", "money")).toBe("R$ 0,1234");
    expect(maskDecimal("R$ 1.234,56", "money")).toBe("R$ 1.234,56");
    expect(maskDecimal("1234,5", "money", "USD")).toBe("US$ 1.234,50");
    expect(maskDecimal("US$ 12,3456", "money", "USD")).toBe("US$ 12,3456");
  });
  it("formata quantidade", () => {
    expect(maskDecimal("1500", "quantity")).toBe("1.500");
    expect(maskDecimal("0,5", "quantity")).toBe("0,5");
    expect(maskDecimal("1234567", "quantity")).toBe("1.234.567");
  });
  it("mantém texto inválido", () => {
    expect(maskDecimal("abc", "money")).toBe("abc");
  });
});

describe("cashMask", () => {
  it("preenche da direita para a esquerda", () => {
    expect(cashMask("1", 2)).toBe("R$ 0,01");
    expect(cashMask("1234", 2)).toBe("R$ 12,34");
    expect(cashMask("123456789", 2)).toBe("R$ 1.234.567,89");
    expect(cashMask("R$ 0,012", 2)).toBe("R$ 0,12");
    expect(cashMask("1", 4, "USD")).toBe("US$ 0,0001");
    expect(cashMask("1501234", 4, "USD")).toBe("US$ 150,1234");
  });
  it("ignora letras e fica vazio sem dígitos", () => {
    expect(cashMask("a1b2", 2)).toBe("R$ 0,12");
    expect(cashMask("R$ 0,00", 2)).toBe("");
    expect(parseDecimalInput(cashMask("1234", 2))).toBe("12.34");
    expect(parseDecimalInput(cashMask("1501234", 4, "USD"))).toBe("150.1234");
  });
  it("carrega um valor gravado", () => {
    expect(toCashMask("12.5", 2)).toEqual({ text: "R$ 12,50", decimals: 2 });
    expect(toCashMask("50.0033", 2)).toEqual({ text: "R$ 50,0033", decimals: 4 });
    expect(toCashMask("100", 4, "USD")).toEqual({ text: "US$ 100,0000", decimals: 4 });
  });
});

describe("money", () => {
  it("formata reais e dólares", () => {
    expect(money("1234.5")).toBe("R$ 1.234,50");
    expect(money("1234.5", "USD")).toBe("US$ 1.234,50");
  });
});
