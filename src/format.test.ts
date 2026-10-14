import { describe, expect, it } from "vitest";
import { date, parseDecimalInput, passwordStrength, percent, shortHash } from "./format";

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
