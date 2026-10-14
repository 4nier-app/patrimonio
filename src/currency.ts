import type { Asset, Currency } from "./api";

/**
 * Moeda do ativo digitado: pelo catálogo quando o código está nele; senão pelo
 * formato (códigos da B3 terminam em número, como ABCD3 ou ABCD11).
 */
export function currencyOfAsset(ticker: string, catalog: Asset[]): Currency | null {
  const t = ticker.trim().toUpperCase();
  if (!t) return null;
  const found = catalog.find((a) => a.ticker === t);
  if (found) return found.currency === "USD" ? "USD" : "BRL";
  if (/\d$/.test(t)) return "BRL";
  if (/^[A-Z][A-Z.-]*$/.test(t)) return "USD";
  return null;
}
