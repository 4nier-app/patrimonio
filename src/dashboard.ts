// Cálculos do painel feitos na tela: recorte do período e encadeamento.
// Os valores exatos e a memória de cálculo vêm do núcleo Rust.
import type { DashMonth } from "./api";

export type Period = "12m" | "ano" | "inicio";

export const PERIODS: { id: Period; label: string }[] = [
  { id: "12m", label: "12 meses" },
  { id: "ano", label: "Este ano" },
  { id: "inicio", label: "Desde o início" },
];

/** Índices dos meses que entram no período escolhido. */
export function periodRange(months: DashMonth[], period: Period): [number, number] {
  const n = months.length;
  if (n === 0) return [0, 0];
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
