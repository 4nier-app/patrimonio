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
 * agrupa. Grupo e código de Bens e Direitos: certos, sugeridos (para conferir)
 * ou digitados pela pessoa, que valem sobre os outros.
 */

const n = (v: string | null | undefined) => (v == null ? 0 : Number(v));
const cents = (v: number) => Math.round(v * 100) / 100;

/**
 * Grupo e código de Bens e Direitos. `sure`: o tipo do ativo deixa claro
 * (ações, FIIs, criptomoedas, FMP-FGTS, conta corrente). Sem `sure`, é uma
 * sugestão tirada da tabela da Receita nos guias de IR dos bancos e
 * corretoras: a tela pede para conferir.
 */
export type BensCode = { group: string; code: string; label: string; sure: boolean };

const sure = (group: string, code: string, label: string): BensCode => ({ group, code, label, sure: true });
const guess = (group: string, code: string, label: string): BensCode => ({ group, code, label, sure: false });

/** Grupo e código de um ativo: certo, sugestão ou null quando o app não sabe. */
export function bensCode(
  asset: string,
  kind: string | undefined,
  opt: { usd?: boolean; name?: string } = {},
): BensCode | null {
  if (kind === "CRIPTO") {
    const a = asset.toUpperCase();
    if (a === "BTC") return sure("08", "01", "Bitcoin");
    if (a === "USDT" || a === "USDC" || a === "DAI") return sure("08", "03", "Stablecoins");
    return sure("08", "02", "Outras criptomoedas");
  }
  if (kind === "ACAO") return sure("03", "01", "Ações");
  if (kind === "FII") return sure("07", "03", "Fundos imobiliários");
  // No exterior (Lei 14.754): ETFs e fundos vão em "Fundos de investimento no exterior".
  if (opt.usd && (kind === "ETF" || kind === "ETF_RF" || kind === undefined))
    return guess("07", "99", "Fundos de investimento no exterior");
  if (kind === "FUNDO") {
    if (/PRIVATIZA/i.test(opt.name ?? "")) return sure("07", "04", "Fundos de ações e FMP-FGTS");
    return guess("07", "01", "Fundos com come-cotas (renda fixa, multimercado, cambial)");
  }
  if (kind === "ETF_RF") return guess("07", "08", "Fundos de índice de renda fixa");
  if (kind === "BDR") return guess("04", "04", "BDRs e outros ativos negociados em bolsa");
  if (kind === "TESOURO") return guess("04", "02", "Títulos públicos e privados sujeitos à tributação");
  return null;
}

/** Grupo e código de um saldo (conta, cofrinho, dólar) ou de um bem (imóvel, veículo). */
export function bensCodeSaldo(kind: string, taxFree = false): BensCode | null {
  if (kind === "Conta corrente") return sure("06", "01", "Depósito em conta corrente");
  if (kind === "Saldo em dólar") return guess("06", "01", "Depósito em conta corrente ou conta pagamento");
  if (kind === "Cofrinho" || kind === "Renda fixa")
    return taxFree
      ? guess("04", "03", "Títulos isentos de tributação (LCI, LCA e outros)")
      : guess("04", "02", "Títulos sujeitos à tributação (CDB, RDB, Tesouro e outros)");
  // Imóvel e veículo: o código depende do tipo (apartamento, casa, terreno...).
  return null;
}

/** Código digitado pela pessoa ("07-99"), que vale sobre a sugestão do app. */
export function savedCode(saved: string | undefined): BensCode | null {
  const m = saved?.match(/^(\d{2})-(\d{2})$/);
  return m ? { group: m[1], code: m[2], label: "Digitado por você", sure: true } : null;
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
