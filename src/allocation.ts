// Alocação ideal: quanto há hoje em cada classe, a meta de cada uma e onde
// colocar o próximo aporte sem vender nada.

import type { EvolutionView } from "./api";

export type ClassId = "fundos" | "b3" | "etf" | "fii" | "exterior" | "cripto" | "rf" | "fgts" | "dinheiro";

export const CLASSES: { id: ClassId; label: string }[] = [
  { id: "fundos", label: "Fundos" },
  { id: "b3", label: "Ações e BDRs na B3" },
  { id: "etf", label: "ETFs na B3" },
  { id: "fii", label: "Fundos imobiliários e Fiagros" },
  { id: "exterior", label: "Ações e ETFs no exterior" },
  { id: "cripto", label: "Criptomoedas" },
  { id: "rf", label: "Renda fixa" },
  { id: "fgts", label: "FGTS" },
  { id: "dinheiro", label: "Dinheiro" },
];

/** O FGTS não recebe aporte: o depósito é do empregador. */
export const NO_DEPOSIT: ClassId[] = ["fgts"];

/** Valor de hoje de cada classe (a coluna "hoje" da Evolução) e se faltou cotação em alguma linha. */
export function currentValues(view: EvolutionView): { values: Record<ClassId, number>; missing: boolean } {
  const values = Object.fromEntries(CLASSES.map((c) => [c.id, 0])) as Record<ClassId, number>;
  const col = todayColumn(view);
  let missing = false;
  for (const g of view.groups) {
    if (!(g.id in values)) continue;
    for (const r of g.rows) {
      const v = r.values[col];
      if (r.missing[col]) missing = true;
      if (v != null) values[g.id as ClassId] += Number(v);
    }
  }
  return { values, missing };
}

/** Coluna de hoje da Evolução; num ano passado, a última (31/12). */
export function todayColumn(view: EvolutionView): number {
  const col = view.columns.findIndex((c) => c.kind === "TODAY");
  return col < 0 ? view.columns.length - 1 : col;
}

/** Quantos ativos do grupo têm valor na coluna (os já vendidos não contam). */
export function heldCount(group: EvolutionView["groups"][number], col: number): number {
  return group.rows.filter((r) => r.missing[col] || (r.values[col] != null && Number(r.values[col]) > 0)).length;
}

/** "1 ativo", "10 ativos". */
export function assetsLabel(n: number): string {
  return n === 1 ? "1 ativo" : `${n} ativos`;
}

export type Line = {
  id: ClassId;
  label: string;
  value: number;
  /** % do total das classes com meta; null quando a classe não tem meta. */
  share: number | null;
  target: number | null;
  /** Quanto falta (positivo) ou sobra (negativo) para chegar na meta hoje. */
  gap: number | null;
  /** Parte do aporte que vai para esta classe. */
  deposit: number;
};

const cents = (n: number) => Math.round(n * 100) / 100;

/**
 * Divide o aporte entre as classes abaixo da meta, sem vender nada: a classe
 * que já passou da meta não recebe, e o resto é dividido para que, depois do
 * aporte, todas fiquem o mais perto possível da meta.
 */
export function distribute(values: Record<ClassId, number>, targets: Partial<Record<ClassId, number>>, amount: number): Partial<Record<ClassId, number>> {
  const out: Partial<Record<ClassId, number>> = {};
  const ids = (Object.keys(targets) as ClassId[]).filter((id) => (targets[id] ?? 0) > 0);
  if (amount <= 0 || !ids.length) return out;
  // Quem não pode receber entra só com o valor que já tem.
  let open = ids.filter((id) => !NO_DEPOSIT.includes(id));
  if (!open.length) return out;
  const total = ids.reduce((s, id) => s + values[id], 0) + amount;
  for (;;) {
    const lockedValue = ids.filter((id) => !open.includes(id)).reduce((s, id) => s + values[id], 0);
    const shareOpen = open.reduce((s, id) => s + (targets[id] ?? 0), 0);
    const pool = total - lockedValue;
    const want = Object.fromEntries(open.map((id) => [id, ((targets[id] ?? 0) / shareOpen) * pool - values[id]]));
    const over = open.filter((id) => want[id] < 0);
    if (!over.length) {
      let left = cents(amount);
      open.forEach((id, i) => {
        const v = cents(i === open.length - 1 ? left : want[id]);
        left = cents(left - v);
        if (v > 0) out[id] = v;
      });
      return out;
    }
    open = open.filter((id) => !over.includes(id));
  }
}

export function lines(values: Record<ClassId, number>, targets: Partial<Record<ClassId, number>>, amount: number): Line[] {
  const withTarget = CLASSES.filter((c) => targets[c.id] != null);
  const total = withTarget.reduce((s, c) => s + values[c.id], 0);
  const deposits = distribute(values, targets, amount);
  return CLASSES.map((c) => {
    const target = targets[c.id] ?? null;
    return {
      id: c.id,
      label: c.label,
      value: values[c.id],
      share: target == null || total <= 0 ? null : (values[c.id] / total) * 100,
      target,
      gap: target == null ? null : cents((target / 100) * total - values[c.id]),
      deposit: deposits[c.id] ?? 0,
    };
  });
}
