import { describe, expect, it } from "vitest";
import type { Asset } from "./api";
import { currencyOfAsset, currencyOfNewAccount } from "./currency";

const asset = (ticker: string, currency: string): Asset => ({
  ticker,
  name: "",
  kind: "ACAO",
  market: currency === "USD" ? "US" : "B3",
  currency,
  isin: null,
  source: "teste",
});

describe("currencyOfAsset", () => {
  it("usa o catálogo quando o código está nele", () => {
    expect(currencyOfAsset("abcd", [asset("ABCD", "USD")])).toBe("USD");
    expect(currencyOfAsset("TEST3", [asset("TEST3", "BRL")])).toBe("BRL");
  });
  it("deduz pelo formato do código", () => {
    expect(currencyOfAsset("ABCD11", [])).toBe("BRL");
    expect(currencyOfAsset("XYZ.B", [])).toBe("USD");
    expect(currencyOfAsset("", [])).toBeNull();
  });
});

describe("currencyOfNewAccount", () => {
  it("corretora do exterior fica em dólar pelo nome", () => {
    expect(currencyOfNewAccount("Nomad", "", [])).toBe("USD");
    expect(currencyOfNewAccount("  NOMAD ", "ABCD3", [])).toBe("USD");
    expect(currencyOfNewAccount("Avenue Securities", "", [])).toBe("USD");
  });
  it("sem pista no nome usa o ativo e, por fim, reais", () => {
    expect(currencyOfNewAccount("Itaú", "ABCD3", [])).toBe("BRL");
    expect(currencyOfNewAccount("Corretora Y", "XYZ", [])).toBe("USD");
    expect(currencyOfNewAccount("Itaú", "", [])).toBe("BRL");
  });
});
