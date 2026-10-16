import type { Asset, Currency } from "./api";

/**
 * Código como o aplicativo grava: maiúsculas, sem espaços, e o do mercado
 * fracionário da B3 ("ABCD3F") no do lote padrão ("ABCD3").
 */
export function normalizeTicker(ticker: string): string {
  const t = ticker.trim().toUpperCase();
  return /^[A-Z]{4}\d{1,2}F$/.test(t) ? t.slice(0, -1) : t;
}

/**
 * Moeda do ativo digitado: pelo catálogo quando o código está nele; senão pelo
 * formato (códigos da B3 terminam em número, como ABCD3 ou ABCD11).
 */
export function currencyOfAsset(ticker: string, catalog: Asset[]): Currency | null {
  const t = normalizeTicker(ticker);
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

/** CNPJ com os dígitos verificadores certos, formatado como o aplicativo grava. */
export function fundTicker(text: string): string | null {
  const t = text.trim();
  if (!/^[\d./\s-]+$/.test(t)) return null;
  const d = t.replace(/\D/g, "");
  if (d.length !== 14 || /^(\d)\1+$/.test(d)) return null;
  const digit = (n: number) => {
    let sum = 0;
    for (let i = 0; i < n; i++) sum += Number(d[n - 1 - i]) * ((i % 8) + 2);
    const r = sum % 11;
    return r < 2 ? 0 : 11 - r;
  };
  if (digit(12) !== Number(d[12]) || digit(13) !== Number(d[13])) return null;
  return `${d.slice(0, 2)}.${d.slice(2, 5)}.${d.slice(5, 8)}/${d.slice(8, 12)}-${d.slice(12)}`;
}

export function isFund(ticker: string): boolean {
  return /^\d{2}\.\d{3}\.\d{3}\/\d{4}-\d{2}$/.test(ticker);
}

/** Nome curto para listas e gráficos: fundo pelo nome, o resto pelo código. */
export function assetLabel(asset: string, names?: Record<string, string>): string {
  const name = names?.[asset];
  if (!isFund(asset) || !name) return asset;
  return name.length > 32 ? `${name.slice(0, 31)}…` : name;
}
