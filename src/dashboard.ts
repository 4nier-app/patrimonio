// Cálculos do painel feitos na tela: recorte do período e encadeamento.
// Os valores exatos e a memória de cálculo vêm do núcleo Rust.
import type { DashMonth } from "./api";

export type Preset = "12m" | "ano" | "inicio";
/** Período pronto, um ano escolhido ou meses escolhidos, de `from` até `to` ("2025-10"). */
export type Period = Preset | { year: number } | { from: string; to: string };

export const PERIODS: { id: Preset; label: string }[] = [
  { id: "12m", label: "12 meses" },
  { id: "ano", label: "Este ano" },
  { id: "inicio", label: "Desde o início" },
];

export function monthKey(m: { year: number; month: number }): string {
  return `${m.year}-${String(m.month).padStart(2, "0")}`;
}

/** Índices dos meses que entram no período escolhido. */
export function periodRange(months: DashMonth[], period: Period): [number, number] {
  const n = months.length;
  if (n === 0) return [0, 0];
  if (typeof period === "object" && "year" in period) {
    return periodRange(months, { from: `${period.year}-01`, to: `${period.year}-12` });
  }
  if (typeof period === "object") {
    const [from, to] = period.from <= period.to ? [period.from, period.to] : [period.to, period.from];
    const a = months.findIndex((m) => monthKey(m) >= from);
    if (a < 0) return [n, n];
    let b = a;
    while (b < n && monthKey(months[b]) <= to) b++;
    return [a, b];
  }
  if (period === "12m") return [Math.max(0, n - 12), n];
  if (period === "ano") {
    const year = months[n - 1].year;
    const first = months.findIndex((m) => m.year === year);
    return [first, n];
  }
  return [0, n];
}

/** Acumulado mês a mês: (1 + r₁)·(1 + r₂)·… − 1; para de desenhar quando falta um mês. */
export function cumulative(rates: (string | null)[]): (number | null)[] {
  let level = 1;
  let broken = false;
  return rates.map((r) => {
    if (r == null || broken) {
      broken = true;
      return null;
    }
    level *= 1 + Number(r);
    return level - 1;
  });
}

export function chained(rates: (string | null)[]): number | null {
  const c = cumulative(rates);
  return c.length ? c[c.length - 1] : null;
}

const SHORT = ["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"];
export function monthTag(m: { year: number; month: number }): string {
  return `${SHORT[m.month - 1]}/${String(m.year).slice(2)}`;
}

const LONG = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];
export function monthName(m: { year: number; month: number }): string {
  return `${LONG[m.month - 1]} de ${m.year}`;
}

/** Soma de lucros por ano, na ordem dos meses. */
export function profitsByYear(months: DashMonth[]): { year: number; profit: number | null }[] {
  const out: { year: number; profit: number | null }[] = [];
  for (const m of months) {
    let row = out.find((r) => r.year === m.year);
    if (!row) {
      row = { year: m.year, profit: 0 };
      out.push(row);
    }
    row.profit = row.profit == null || m.profit == null ? null : row.profit + Number(m.profit);
  }
  return out;
}
