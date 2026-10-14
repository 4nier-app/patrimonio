import { describe, expect, it } from "vitest";
import type { Asset } from "./api";
import { currencyOfAsset } from "./currency";

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
