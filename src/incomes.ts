import type { IncomeKind, IncomeRow } from "./api";

export const KIND_LABEL: Record<IncomeKind, string> = {
  DIVIDENDO: "Dividendo",
  JCP: "JCP",
  RENDIMENTO: "Rendimento",
  JUROS: "Juros",
  OUTRO: "Outro",
};

export const KINDS: IncomeKind[] = ["DIVIDENDO", "JCP", "RENDIMENTO", "JUROS", "OUTRO"];

/** Tipo sugerido pelo ativo: FII paga rendimento; o resto, dividendo. */
export function defaultKind(assetKind: string | undefined): IncomeKind {
  return assetKind === "FII" ? "RENDIMENTO" : "DIVIDENDO";
}

const n = (v: string | null) => (v == null ? 0 : Number(v));
const cents = (v: number) => Math.round(v * 100) / 100;

export type Totals = { gross: number; withheld: number; net: number; missingFx: number };

/** Soma em reais; proventos sem PTAX ficam de fora e são contados à parte. */
export function totals(rows: IncomeRow[]): Totals {
  const t: Totals = { gross: 0, withheld: 0, net: 0, missingFx: 0 };
  for (const r of rows) {
    if (r.net_brl == null) {
      t.missingFx++;
      continue;
    }
    t.gross += n(r.gross_brl);
    t.withheld += n(r.withheld_brl);
    t.net += n(r.net_brl);
  }
  return { gross: cents(t.gross), withheld: cents(t.withheld), net: cents(t.net), missingFx: t.missingFx };
}

export function years(rows: IncomeRow[]): number[] {
  return [...new Set(rows.map((r) => Number(r.income.pay_date.slice(0, 4))))].sort((a, b) => b - a);
}

export function inYear(rows: IncomeRow[], year: number | null): IncomeRow[] {
  return year == null ? rows : rows.filter((r) => r.income.pay_date.startsWith(String(year)));
}

/** Líquido em reais por mês do ano (12 posições) e por tipo. */
export function byMonth(rows: IncomeRow[], year: number): Record<IncomeKind, number[]> {
  const out = Object.fromEntries(KINDS.map((k) => [k, Array(12).fill(0)])) as Record<IncomeKind, number[]>;
  for (const r of inYear(rows, year)) {
    const m = Number(r.income.pay_date.slice(5, 7)) - 1;
    out[r.income.kind][m] = cents(out[r.income.kind][m] + n(r.net_brl));
  }
  return out;
}

/** Líquido em reais por ano e por tipo, do ano mais antigo ao mais recente. */
export function byYear(rows: IncomeRow[]): { years: number[]; values: Record<IncomeKind, number[]> } {
  const ys = years(rows).reverse();
  const values = Object.fromEntries(KINDS.map((k) => [k, ys.map(() => 0)])) as Record<IncomeKind, number[]>;
  for (const r of rows) {
    const i = ys.indexOf(Number(r.income.pay_date.slice(0, 4)));
    values[r.income.kind][i] = cents(values[r.income.kind][i] + n(r.net_brl));
  }
  return { years: ys, values };
}

export type AssetIncome = { asset: string; net: number; gross: number; count: number; last: string };

/** Total líquido por ativo, do maior para o menor. */
export function byAsset(rows: IncomeRow[]): AssetIncome[] {
  const map = new Map<string, AssetIncome>();
  for (const r of rows) {
    const a = map.get(r.income.asset) ?? { asset: r.income.asset, net: 0, gross: 0, count: 0, last: "" };
    a.net = cents(a.net + n(r.net_brl));
    a.gross = cents(a.gross + n(r.gross_brl));
    a.count++;
    if (r.income.pay_date > a.last) a.last = r.income.pay_date;
    map.set(r.income.asset, a);
  }
  return [...map.values()].sort((a, b) => b.net - a.net);
}

/** Proventos líquidos dos últimos 12 meses até `today` (inclusive). */
export function last12(rows: IncomeRow[], today: string): IncomeRow[] {
  const [y, m, d] = today.split("-").map(Number);
  const from = new Date(Date.UTC(y - 1, m - 1, d + 1)).toISOString().slice(0, 10);
  return rows.filter((r) => r.income.pay_date >= from && r.income.pay_date <= today);
}

/**
 * Resumo para a declaração do ano, por grupo da Receita. Dividendos e
 * rendimentos de FII no Brasil são isentos; JCP e juros já vêm com o imposto
 * retido (tributação exclusiva). No exterior (Lei 14.754/2023), os proventos
 * entram no ajuste anual a 15%, e o imposto retido lá fora pode ser
 * descontado; a conta aqui é só uma estimativa.
 */
export type TaxGroup = { id: "isento" | "exclusivo" | "exterior"; gross: number; withheld: number; net: number; count: number };

export function taxGroups(rows: IncomeRow[]): TaxGroup[] {
  const groups: Record<TaxGroup["id"], TaxGroup> = {
    isento: { id: "isento", gross: 0, withheld: 0, net: 0, count: 0 },
    exclusivo: { id: "exclusivo", gross: 0, withheld: 0, net: 0, count: 0 },
    exterior: { id: "exterior", gross: 0, withheld: 0, net: 0, count: 0 },
  };
  for (const r of rows) {
    const i = r.income;
    const id: TaxGroup["id"] =
      i.currency === "USD" ? "exterior" : i.kind === "JCP" || i.kind === "JUROS" ? "exclusivo" : "isento";
    const g = groups[id];
    g.gross = cents(g.gross + n(r.gross_brl));
    g.withheld = cents(g.withheld + n(r.withheld_brl));
    g.net = cents(g.net + n(r.net_brl));
    g.count++;
  }
  return Object.values(groups).filter((g) => g.count > 0);
}

/** Imposto estimado no ajuste anual sobre proventos do exterior: 15% menos o já retido, nunca negativo. */
export function foreignTaxDue(g: TaxGroup): number {
  return Math.max(0, cents(g.gross * 0.15 - g.withheld));
}

export type ProjectedAsset = {
  asset: string;
  /** Líquido recebido nos últimos 12 meses. */
  received: number;
  /** Cotas hoje sobre as cotas de quando recebeu (média ponderada); null sem posição registrada. */
  factor: number | null;
  /** Cotas de hoje, somando as contas; null sem posição registrada. */
  held: number | null;
  projected: number;
};

export type Projection = {
  /** 12 meses, a partir do mês que vem: "AAAA-MM". */
  months: string[];
  values: number[];
  total: number;
  received: number;
  assets: ProjectedAsset[];
  missingFx: number;
};

/**
 * Estimativa dos próximos 12 meses: cada provento líquido dos últimos 12
 * meses se repete no mesmo mês, ajustado pela quantidade que você tem hoje
 * (comprou mais, recebe mais; vendeu tudo, não recebe). Proventos sem
 * posição registrada (juros de saldo, por exemplo) se repetem como foram.
 */
export function projection(rows: IncomeRow[], today: string, held: Record<string, number>): Projection {
  const [y, m] = today.split("-").map(Number);
  const months = Array.from({ length: 12 }, (_, i) => {
    const d = new Date(Date.UTC(y, m + i, 1));
    return d.toISOString().slice(0, 7);
  });
  const values = Array(12).fill(0);
  const map = new Map<string, { received: number; projected: number; weighted: number; base: number }>();
  let missingFx = 0;
  for (const r of last12(rows, today)) {
    if (r.net_brl == null) {
      missingFx++;
      continue;
    }
    const net = n(r.net_brl);
    const q = r.quantity == null ? 0 : n(r.quantity);
    const factor = q > 0 ? (held[r.income.asset] ?? 0) / q : 1;
    const v = net * factor;
    const i = months.findIndex((k) => k.slice(5, 7) === r.income.pay_date.slice(5, 7));
    values[i] += v;
    const a = map.get(r.income.asset) ?? { received: 0, projected: 0, weighted: 0, base: 0 };
    a.received += net;
    a.projected += v;
    if (q > 0) {
      a.weighted += factor * net;
      a.base += net;
    }
    map.set(r.income.asset, a);
  }
  const assets = [...map.entries()]
    .map(([asset, a]) => ({
      asset,
      received: cents(a.received),
      factor: a.base > 0 ? a.weighted / a.base : null,
      held: a.base > 0 ? (held[asset] ?? 0) : null,
      projected: cents(a.projected),
    }))
    .sort((a, b) => b.projected - a.projected || b.received - a.received);
  return {
    months,
    values: values.map(cents),
    total: cents(values.reduce((s, v) => s + v, 0)),
    received: cents(assets.reduce((s, a) => s + a.received, 0)),
    assets,
    missingFx,
  };
}
