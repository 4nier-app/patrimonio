const dateTime = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "medium" });

/** Data/hora ISO (UTC) no fuso e formato do usuário. */
export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateTime.format(d);
}

/** Primeiros caracteres de um hash, para exibição. */
export function shortHash(hash: string, size = 10): string {
  return hash.length > size ? `${hash.slice(0, size)}…` : hash;
}

/** Força da senha numa escala de 0 a 4, para orientar o usuário. */
export function passwordStrength(password: string): number {
  let score = 0;
  if (password.length >= 8) score++;
  if (password.length >= 12) score++;
  if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score++;
  if (/\d/.test(password) && /[^A-Za-z0-9]/.test(password)) score++;
  return score;
}

// Valores chegam do núcleo como texto decimal exato ("1234.5"); a conversão
// para número aqui serve só para exibir, nunca para calcular.
const brlFmt = new Intl.NumberFormat("pt-BR", { style: "currency", currency: "BRL" });
const usdFmt = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const qtyFmt = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 8 });
const pctFmt = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const priceFmt = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 8 });

export type Currency = "BRL" | "USD";
const SYMBOL: Record<Currency, string> = { BRL: "R$", USD: "US$" };

export function money(v: string | null | undefined, currency: Currency = "BRL"): string {
  if (v == null) return "—";
  return currency === "USD" ? `US$ ${usdFmt.format(Number(v))}` : brlFmt.format(Number(v));
}

/** Preço médio: 2 casas, como nas corretoras (o valor exato fica na dica). */
export function averagePrice(v: string | null | undefined, currency: Currency = "BRL"): string {
  return money(v, currency);
}

export function price(v: string | null | undefined, currency: Currency = "BRL"): string {
  return v == null ? "—" : `${SYMBOL[currency]} ${priceFmt.format(Number(v))}`;
}

export function quantity(v: string | null | undefined): string {
  return v == null ? "—" : qtyFmt.format(Number(v));
}

/** Fração como percentual: "0.0523" → "5,23%". */
export function percent(v: string | null | undefined): string {
  return v == null ? "—" : `${pctFmt.format(Number(v) * 100)}%`;
}

/** Sinal do valor para colorir ganhos e perdas. */
export function tone(v: string | null | undefined): "" | "pos" | "neg" {
  if (v == null) return "";
  const n = Number(v);
  return n > 0 ? "pos" : n < 0 ? "neg" : "";
}

/** "2026-03-02" → "02/03/2026". */
export function date(iso: string): string {
  const [y, m, d] = iso.split("-");
  return d && m && y ? `${d}/${m}/${y}` : iso;
}

const MONTHS = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
export function monthLabel(month: number): string {
  return MONTHS[month - 1] ?? String(month);
}

/**
 * Número digitado no padrão brasileiro para texto decimal exato.
 * "1.234,56" → "1234.56"; "12,5" → "12.5"; "12.5" → "12.5". Inválido → null.
 */
export function parseDecimalInput(raw: string, dotIsDecimal = false): string | null {
  let s = raw.trim().replace(/\s/g, "").replace(/^(R|US)\$/, "");
  if (!s) return null;
  if (s.includes(",")) s = s.replace(/\./g, "").replace(",", ".");
  // Conta no exterior: as notas usam ponto decimal ("1.234" = um vírgula dois três quatro).
  else if (dotIsDecimal && s.indexOf(".") === s.lastIndexOf(".")) return /^\d+(\.\d+)?$/.test(s) ? s : null;
  else if (/^\d{1,3}(\.\d{3})+$/.test(s)) s = s.replace(/\./g, "");
  return /^\d+(\.\d+)?$/.test(s) ? s : null;
}

/**
 * Máscara aplicada ao sair do campo: "1234,5" → "R$ 1.234,50" (dinheiro) ou
 * "1500" → "1.500" (quantidade). Trabalha só com texto, sem arredondar.
 * Texto inválido volta como foi digitado, para o usuário corrigir.
 */
export function maskDecimal(
  raw: string,
  kind: "money" | "quantity",
  currency: Currency = "BRL",
  dotIsDecimal = false,
): string {
  const dec = parseDecimalInput(raw, dotIsDecimal);
  if (dec == null) return raw;
  const [intPart, frac = ""] = dec.split(".");
  const trimmed = intPart.replace(/^0+(?=\d)/, "") || "0";
  // Sem milhar quando o ponto é decimal, para "1.234" não mudar de sentido.
  const grouped = dotIsDecimal ? trimmed : trimmed.replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  if (kind === "money") return `${SYMBOL[currency]} ${grouped},${frac.padEnd(2, "0")}`;
  return frac ? `${grouped},${frac}` : grouped;
}

/** Deixa passar só o que pode compor um número: dígitos, vírgula, ponto e o símbolo da moeda. */
export function keepNumberChars(raw: string): string {
  return raw.replace(/[^\d.,RUS$\s]/g, "");
}

/** Casas decimais do preço: reais com 2 (máscara de caixa); dólar até 8 (preço médio nas notas da Nomad). */
export function priceDecimals(currency: Currency): number {
  return currency === "USD" ? 8 : 2;
}

/**
 * Máscara de caixa: só aceita dígitos e preenche da direita para a esquerda.
 * Com 2 casas, "1" → "R$ 0,01" e "1234" → "R$ 12,34". Com 4 casas,
 * "1" → "US$ 0,0001". Campo sem dígitos fica vazio.
 */
export function cashMask(raw: string, decimals: number, currency: Currency = "BRL"): string {
  const digits = raw.replace(/\D/g, "").replace(/^0+/, "");
  if (!digits) return "";
  const padded = digits.padStart(decimals + 1, "0");
  const intPart = padded.slice(0, padded.length - decimals).replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  return `${SYMBOL[currency]} ${intPart},${padded.slice(padded.length - decimals)}`;
}

/** Zero na máscara de caixa ("R$ 0,00"). */
export function cashZero(decimals: number, currency: Currency = "BRL"): string {
  return `${SYMBOL[currency]} 0,${"0".repeat(decimals)}`;
}

/**
 * Máscara de caixa ao digitar: como `cashMask`, mas digitar 0 num campo vazio
 * (ou mais zeros) dá "R$ 0,00", para informar saldo zero. Apagar volta a
 * deixar o campo vazio.
 */
export function cashTyping(raw: string, previous: string, decimals: number, currency: Currency = "BRL"): string {
  const next = cashMask(raw, decimals, currency);
  const zeros = /0/.test(raw.replace(/\D/g, ""));
  return !next && zeros && raw.length > previous.length ? cashZero(decimals, currency) : next;
}

/**
 * Valor exato ("12.5") na máscara de caixa, para alterar um lançamento.
 * Usa mais casas se o valor gravado tiver mais que o padrão.
 */
export function toCashMask(dec: string, decimals: number, currency: Currency = "BRL"): { text: string; decimals: number } {
  const [intPart, frac = ""] = dec.split(".");
  const d = Math.max(decimals, frac.replace(/0+$/, "").length);
  return { text: cashMask(intPart + frac.padEnd(d, "0").slice(0, d), d, currency), decimals: d };
}

/**
 * Máscara para preço em dólar: números e separador decimal, com até
 * `maxDecimals` casas, sem separador de milhar (as notas do exterior usam
 * ponto decimal). Ponto ou vírgula valem como decimal: "534.7361" e
 * "534,7361" → "US$ 534,7361". Com os dois ("1,234.56" ou "1.234,56"), o
 * último é o decimal; um mesmo sinal repetido ("1.234.567") é de milhar.
 */
export function decimalMask(raw: string, maxDecimals: number, currency: Currency = "USD"): string {
  const s = raw.replace(/^(R|US)\$\s?/, "").replace(/[^\d,.]/g, "");
  if (!s) return "";
  const last = Math.max(s.lastIndexOf(","), s.lastIndexOf("."));
  const sep = last >= 0 ? s[last] : "";
  const both = s.includes(",") && s.includes(".");
  const repeated = sep !== "" && s.indexOf(sep) !== last;
  const decimal = sep !== "" && (both || !repeated) ? last : -1;
  const intPart = (decimal >= 0 ? s.slice(0, decimal) : s).replace(/[,.]/g, "");
  const intDigits = intPart.replace(/^0+(?=\d)/, "") || "0";
  if (decimal < 0) return `${SYMBOL[currency]} ${intDigits}`;
  const frac = s.slice(decimal + 1).replace(/[,.]/g, "").slice(0, maxDecimals);
  return `${SYMBOL[currency]} ${intDigits},${frac}`;
}
