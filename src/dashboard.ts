// Cálculos do painel feitos na tela: recorte do período e encadeamento.
// Os valores exatos e a memória de cálculo vêm do núcleo Rust.
import type { AssetMonths, DashboardView, DashMonth, DaysView } from "./api";

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

/**
 * Acumulado mês a mês: (1 + r₁)·(1 + r₂)·… − 1; para de desenhar quando falta
 * a taxa de um mês (null). Mês sem carteira (undefined) fica em branco e não
 * interrompe o acumulado.
 */
export function cumulative(rates: (string | null | undefined)[]): (number | null)[] {
  let level = 1;
  let broken = false;
  return rates.map((r) => {
    if (r === undefined) return null;
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

/** Mês escolhido sozinho ("2025-10"): o painel mostra os dias dele. */
export function singleMonth(period: Period): { year: number; month: number } | null {
  if (typeof period !== "object" || !("from" in period) || period.from !== period.to) return null;
  const [y, m] = period.from.split("-").map(Number);
  return { year: y, month: m };
}

/**
 * O que os gráficos desenham: um rótulo por posição e, em cada uma, o mês (ou
 * dia) da carteira ou `null` quando não houve carteira. Índices e lucro por
 * ativo vêm alinhados às mesmas posições (undefined onde não há carteira).
 */
export type Frame = {
  days: boolean;
  labels: string[];
  slots: (DashMonth | null)[];
  bench: { name: string; rates: (string | null | undefined)[]; same_flows: (string | null | undefined)[] }[];
  contributions: AssetMonths[];
};

function pick<T>(list: T[], at: number[]): (T | undefined)[] {
  return at.map((i) => (i < 0 ? undefined : list[i]));
}

/** Meses do período; um ano escolhido sempre tem os 12 meses. */
export function monthFrame(view: DashboardView, period: Period): Frame {
  let at: number[];
  let labels: string[];
  if (typeof period === "object" && "year" in period) {
    at = Array.from({ length: 12 }, (_, k) => view.months.findIndex((m) => m.year === period.year && m.month === k + 1));
    labels = at.map((_, k) => monthTag({ year: period.year, month: k + 1 }));
  } else {
    const [a, b] = periodRange(view.months, period);
    at = Array.from({ length: Math.max(0, b - a) }, (_, k) => a + k);
    labels = at.map((i) => monthTag(view.months[i]));
  }
  return {
    days: false,
    labels,
    slots: at.map((i) => (i < 0 ? null : view.months[i])),
    bench: view.benchmarks.map((b) => ({ name: b.name, rates: pick(b.rates, at), same_flows: pick(b.same_flows, at) })),
    contributions: view.contributions.map((c) => ({ asset: c.asset, profits: pick(c.profits, at).map((v) => v ?? null) })),
  };
}

/** Dias úteis de um mês, rotulados "dd/mm". */
export function dayFrame(view: DaysView): Frame {
  return {
    days: true,
    labels: view.days.map((d) => `${d.period_end.slice(8, 10)}/${d.period_end.slice(5, 7)}`),
    slots: view.days,
    bench: view.benchmarks.map((b) => ({ name: b.name, rates: b.rates, same_flows: b.same_flows })),
    contributions: view.contributions,
  };
}
