// Meta de patrimônio: quanto falta, quanto guardar por mês e o ritmo dos
// últimos 12 meses.

export type Point = { date: string; value: number };

const DAY = 86_400_000;
const MONTH_DAYS = 365.25 / 12;

const days = (from: string, to: string) => (Date.parse(to) - Date.parse(from)) / DAY;

function addMonths(iso: string, months: number): string {
  const d = new Date(Date.parse(iso) + months * MONTH_DAYS * DAY);
  return d.toISOString().slice(0, 10);
}

export type Progress = {
  /** Fração já alcançada (0 a 1, ou mais se passou da meta). */
  done: number;
  missing: number;
  /** Meses até a data da meta. */
  monthsLeft: number;
  /** Quanto o patrimônio precisa crescer por mês até a data, sem contar rendimento. */
  perMonth: number | null;
  /** Crescimento médio por mês no último ano (ou desde o ponto mais antigo, se for menos). */
  pace: number | null;
  /** Período do ritmo, em meses. */
  paceMonths: number | null;
  /** Quando chega lá mantendo o ritmo. */
  arrival: string | null;
};

/**
 * `history` são os valores do patrimônio no fim de cada mês (e hoje), em
 * qualquer ordem. O ritmo usa o ponto mais perto de 12 meses atrás, e só
 * existe com pelo menos 2 meses de histórico.
 */
export function progress(now: Point, target: { value: number; date: string }, history: Point[]): Progress {
  const missing = Math.max(0, target.value - now.value);
  const monthsLeft = Math.max(0, days(now.date, target.date) / MONTH_DAYS);
  const yearAgo = addMonths(now.date, -12);
  const older = history.filter((p) => p.date < now.date && days(p.date, now.date) >= 2 * MONTH_DAYS);
  // O ponto mais perto de 12 meses atrás (com pouco histórico, o mais antigo).
  const gap = (p: Point) => Math.abs(days(p.date, yearAgo));
  const start = older.sort((a, b) => gap(a) - gap(b))[0];
  const paceMonths = start ? days(start.date, now.date) / MONTH_DAYS : null;
  const pace = start && paceMonths ? (now.value - start.value) / paceMonths : null;
  return {
    done: target.value > 0 ? now.value / target.value : 0,
    missing,
    monthsLeft,
    perMonth: missing > 0 && monthsLeft > 0 ? missing / monthsLeft : null,
    pace,
    paceMonths,
    arrival: missing === 0 ? now.date : pace && pace > 0 ? addMonths(now.date, missing / pace) : null,
  };
}
