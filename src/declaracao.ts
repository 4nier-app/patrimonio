import type {
  EventRow,
  IncomeRow,
  SuggestedIncome,
  TaxMonth,
  YearEndPosition,
} from "./api";

/**
 * Resumo para o contador: os dados do ano organizados na ordem das fichas
 * da declaração. Os cálculos são os mesmos das outras telas; aqui só se
 * agrupa. Grupo e código de Bens e Direitos aparecem só quando são certos
 * (ações, FIIs, criptomoedas); nos outros casos o contador confirma.
 */

const n = (v: string | null | undefined) => (v == null ? 0 : Number(v));
const cents = (v: number) => Math.round(v * 100) / 100;

export type BensCode = { group: string; code: string; label: string };

/** Grupo e código de Bens e Direitos, quando o tipo do ativo deixa isso claro. */
export function bensCode(
  asset: string,
  kind: string | undefined,
): BensCode | null {
  if (kind === "ACAO") return { group: "03", code: "01", label: "Ações" };
  if (kind === "FII")
    return { group: "07", code: "03", label: "Fundos imobiliários" };
  if (kind === "CRIPTO") {
    const a = asset.toUpperCase();
    if (a === "BTC") return { group: "08", code: "01", label: "Bitcoin" };
    if (a === "USDT" || a === "USDC" || a === "DAI")
      return { group: "08", code: "03", label: "Stablecoins" };
    return { group: "08", code: "02", label: "Outras criptomoedas" };
  }
  return null;
}

/** Palavra para a quantidade: ações, cotas ou unidades. */
function unit(kind: string | undefined, qty: number): string {
  const one = qty === 1;
  if (kind === "ACAO" || kind === "BDR") return one ? "ação" : "ações";
  if (kind === "CRIPTO") return "";
  if (
    kind === "FII" ||
    kind === "ETF" ||
    kind === "ETF_RF" ||
    kind === "FI_INFRA" ||
    kind === "FUNDO"
  )
    return one ? "cota" : "cotas";
  return one ? "unidade" : "unidades";
}

const br = (v: number, digits = 2) =>
  v.toLocaleString("pt-BR", {
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  });

function qtyText(q: number): string {
  return q.toLocaleString("pt-BR", { maximumFractionDigits: 8 });
}

/** Texto da discriminação: quantidade, ativo, nome e custo médio. */
export function discriminacao(
  p: YearEndPosition,
  kind: string | undefined,
  name: string | undefined,
): string {
  const q = n(p.quantity);
  const u = unit(kind, q);
  const label = name && name !== p.asset ? `${p.asset} (${name})` : p.asset;
  const head = `${qtyText(q)}${u ? ` ${u}` : ""} ${label}`;
  if (p.cost_usd != null) {
    const avg = q > 0 ? n(p.cost_usd) / q : 0;
    return `${head}, custo médio US$ ${br(avg)}, total US$ ${br(n(p.cost_usd))}`;
  }
  const avg = q > 0 ? n(p.cost_brl) / q : 0;
  return `${head}, custo médio R$ ${br(avg)}`;
}

export type IncomeLine = {
  id: string;
  label: string;
  net: number;
  gross: number;
  withheld: number;
  assets: string[];
};

/**
 * Proventos do ano por linha da declaração. No Brasil: dividendos e
 * rendimentos de FIIs são isentos; JCP e juros, tributação exclusiva. Os do
 * exterior ficam à parte (Lei 14.754/2023). Proventos sem PTAX ficam de fora
 * e são contados em `missingFx`.
 */
export function incomeLines(rows: IncomeRow[], year: number) {
  const lines: Record<string, IncomeLine> = {};
  const add = (id: string, label: string, r: IncomeRow) => {
    const l = (lines[id] ??= {
      id,
      label,
      net: 0,
      gross: 0,
      withheld: 0,
      assets: [],
    });
    l.net = cents(l.net + n(r.net_brl));
    l.gross = cents(l.gross + n(r.gross_brl));
    l.withheld = cents(l.withheld + n(r.withheld_brl));
    if (!l.assets.includes(r.income.asset)) l.assets.push(r.income.asset);
  };
  let missingFx = 0;
  for (const r of rows) {
    if (!r.income.pay_date.startsWith(String(year))) continue;
    if (r.net_brl == null) {
      missingFx++;
      continue;
    }
    const i = r.income;
    if (i.currency === "USD") add("exterior", "Proventos no exterior", r);
    else if (i.kind === "JCP") add("jcp", "Juros sobre capital próprio", r);
    else if (i.kind === "JUROS") add("juros", "Juros recebidos", r);
    else if (i.kind === "RENDIMENTO")
      add("fii", "Rendimentos de FIIs e Fiagros", r);
    else add("dividendos", "Lucros e dividendos recebidos", r);
  }
  for (const l of Object.values(lines)) l.assets.sort();
  const pick = (...ids: string[]) =>
    ids.map((id) => lines[id]).filter((l): l is IncomeLine => l != null);
  return {
    isentos: pick("dividendos", "fii"),
    exclusivos: pick("jcp", "juros"),
    exterior: lines.exterior ?? null,
    missingFx,
  };
}

/** Bonificações do ano (pela data ex), com o valor isento de cada uma. */
export function bonificacoes(events: EventRow[], year: number) {
  const list = events.filter(
    (e) =>
      e.event.kind === "BONIFICACAO" &&
      e.ex_date.startsWith(String(year)) &&
      e.exempt_brl != null,
  );
  return {
    total: cents(list.reduce((s, e) => s + n(e.exempt_brl), 0)),
    assets: [...new Set(list.map((e) => e.event.asset))].sort(),
  };
}

export type Pendencia = {
  id: string;
  text: string;
  action?: "proventos" | "impostos";
};

/** O que falta resolver antes de levar ao contador. */
export function pendencias(
  year: number,
  months: TaxMonth[],
  suggestions: SuggestedIncome[],
  missingFx: number,
): Pendencia[] {
  const out: Pendencia[] = [];
  const missing = suggestions.filter((s) => s.ex_date.startsWith(String(year)));
  if (missing.length > 0) {
    const assets = [...new Set(missing.map((s) => s.asset))].sort();
    out.push({
      id: "proventos",
      text: `${missing.length} provento${missing.length > 1 ? "s" : ""} de ${year} que falta${missing.length > 1 ? "m" : ""} lançar (${assets.join(", ")})`,
      action: "proventos",
    });
  }
  for (const m of months) {
    if (!m.month.startsWith(String(year))) continue;
    if (m.status === "PENDENTE" || m.status === "ATRASADO") {
      const name = new Date(`${m.month}-15T12:00:00`).toLocaleDateString(
        "pt-BR",
        { month: "long" },
      );
      out.push({
        id: `darf-${m.month}`,
        text: `DARF de ${name} ${m.status === "ATRASADO" ? "atrasado" : "em aberto"}: R$ ${br(n(m.darf))}`,
        action: "impostos",
      });
    }
  }
  if (missingFx > 0) {
    out.push({
      id: "ptax",
      text: `${missingFx} provento${missingFx > 1 ? "s" : ""} em dólar sem a PTAX do dia (atualize as cotações)`,
    });
  }
  return out;
}
