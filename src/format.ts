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
const qtyFmt = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 8 });
const pctFmt = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
const priceFmt = new Intl.NumberFormat("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 8 });

export function money(v: string | null | undefined): string {
  return v == null ? "—" : brlFmt.format(Number(v));
}

export function price(v: string | null | undefined): string {
  return v == null ? "—" : `R$ ${priceFmt.format(Number(v))}`;
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
export function parseDecimalInput(raw: string): string | null {
  let s = raw.trim().replace(/\s/g, "").replace(/^(R|US)\$/, "");
  if (!s) return null;
  if (s.includes(",")) s = s.replace(/\./g, "").replace(",", ".");
  else if (/^\d{1,3}(\.\d{3})+$/.test(s)) s = s.replace(/\./g, "");
  return /^\d+(\.\d+)?$/.test(s) ? s : null;
}

/**
 * Máscara aplicada ao sair do campo: "1234,5" → "R$ 1.234,50" (dinheiro) ou
 * "1500" → "1.500" (quantidade). Trabalha só com texto, sem arredondar.
 * Texto inválido volta como foi digitado, para o usuário corrigir.
 */
export type Currency = "BRL" | "USD";
const SYMBOL: Record<Currency, string> = { BRL: "R$", USD: "US$" };

export function maskDecimal(raw: string, kind: "money" | "quantity", currency: Currency = "BRL"): string {
  const dec = parseDecimalInput(raw);
  if (dec == null) return raw;
  const [intPart, frac = ""] = dec.split(".");
  const grouped = intPart.replace(/^0+(?=\d)/, "").replace(/\B(?=(\d{3})+(?!\d))/g, ".");
  if (kind === "money") return `${SYMBOL[currency]} ${grouped},${frac.padEnd(2, "0")}`;
  return frac ? `${grouped},${frac}` : grouped;
}

/** Deixa passar só o que pode compor um número: dígitos, vírgula, ponto e o símbolo da moeda. */
export function keepNumberChars(raw: string): string {
  return raw.replace(/[^\d.,RUS$\s]/g, "");
}
