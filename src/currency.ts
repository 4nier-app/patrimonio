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

/** Corretoras do exterior: contas novas com esses nomes operam em dólar. */
const FOREIGN_BROKERS = [
  "nomad",
  "avenue",
  "inter global",
  "interactive brokers",
  "ibkr",
  "charles schwab",
  "schwab",
  "passfolio",
  "stake",
  "drivewealth",
  "apex",
  "c6 global",
  "xp internacional",
  "btg global",
];

function normalize(s: string): string {
  return s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase().trim();
}

/**
 * Moeda de uma conta nova: pelo nome da corretora (Nomad, Avenue... → dólar);
 * se o nome não diz, pelo ativo; sem pista, reais.
 */
export function currencyOfNewAccount(account: string, ticker: string, catalog: Asset[]): Currency {
  const name = normalize(account);
  if (name && FOREIGN_BROKERS.some((b) => name.includes(b))) return "USD";
  return currencyOfAsset(ticker, catalog) ?? "BRL";
}
