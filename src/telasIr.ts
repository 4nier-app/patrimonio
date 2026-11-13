import type {
  EventRow,
  IncomeRow,
  TaxMonth,
  TaxView,
  YearEndPosition,
} from "./api";
import { bensCode, discriminacao } from "./declaracao";

/**
 * Telas do IR: os mesmos valores das outras telas, arrumados como nas
 * fichas do programa do IR. Aqui não há cálculo de imposto novo: a
 * apuração vem de `TaxMonth`; só se reparte o que o programa mostra em
 * campos separados (operações comuns, day trade, FII, IR na fonte de cada
 * tipo).
 *
 * Diferenças assumidas do programa:
 * - o IR na fonte a compensar recomeça em janeiro (o que sobra em dezembro
 *   vai para o ajuste anual, não para o ano seguinte);
 * - o programa não tem o limite de R$ 10 do DARF: "imposto a pagar" é o do
 *   mês, sem o que ficou acumulado de meses anteriores.
 */

const n = (v: string | null | undefined) => (v == null ? 0 : Number(v));
const cents = (v: number) => Math.round(v * 100) / 100;

export type GroupCalc = {
  result: number;
  lossBefore: number;
  base: number;
  lossAfter: number;
  rate: number;
  tax: number;
};

export type RvMonth = {
  month: number;
  hasSales: boolean;
  comum: GroupCalc;
  dt: GroupCalc;
  totalTax: number;
  irrfDtMonth: number;
  irrfDtBefore: number;
  irrfDtComp: number;
  irrfLeiMonth: number;
  irrfLeiBefore: number;
  irrfLeiComp: number;
  toPay: number;
  paid: number;
};

export type FiiMonth = GroupCalc & {
  month: number;
  hasSales: boolean;
  irrfBefore: number;
  irrfMonth: number;
  irrfComp: number;
  toPay: number;
  paid: number;
};

type GroupId = "COMUM" | "DAY_TRADE" | "FII";
const RATES: Record<GroupId, number> = { COMUM: 0.15, DAY_TRADE: 0.2, FII: 0.2 };

/** Prejuízo de cada grupo no fim do mês mais recente antes de `year`. */
function lossAtStart(months: TaxMonth[], year: number, g: GroupId): number {
  const before = months
    .filter((m) => Number(m.month.slice(0, 4)) < year)
    .sort((a, b) => a.month.localeCompare(b.month));
  for (let i = before.length - 1; i >= 0; i--) {
    const x = before[i].groups.find((y) => y.group === g);
    if (x) return n(x.loss_after);
  }
  return 0;
}

function groupCalc(m: TaxMonth | undefined, g: GroupId, carry: number): GroupCalc {
  const x = m?.groups.find((y) => y.group === g);
  if (!x)
    return { result: 0, lossBefore: carry, base: 0, lossAfter: carry, rate: RATES[g], tax: 0 };
  return {
    result: n(x.result),
    lossBefore: n(x.loss_before),
    base: n(x.base),
    lossAfter: n(x.loss_after),
    rate: n(x.rate),
    tax: n(x.tax),
  };
}

/** Usa o IR na fonte disponível para abater o imposto; devolve o que sobrou. */
function use(available: number, tax: number): [used: number, left: number] {
  const used = Math.min(available, tax);
  return [cents(used), cents(available - used)];
}

export function rendaVariavel(
  months: TaxMonth[],
  year: number,
): { comuns: RvMonth[]; fii: FiiMonth[] } {
  const byMonth = new Map(months.map((m) => [m.month, m]));
  let lossC = lossAtStart(months, year, "COMUM");
  let lossD = lossAtStart(months, year, "DAY_TRADE");
  let lossF = lossAtStart(months, year, "FII");
  let dtCarry = 0;
  let leiCarry = 0;
  let fiiCarry = 0;
  const comuns: RvMonth[] = [];
  const fii: FiiMonth[] = [];
  for (let i = 1; i <= 12; i++) {
    const key = `${year}-${String(i).padStart(2, "0")}`;
    const m = byMonth.get(key);
    const sales = m?.sales ?? [];
    const comum = groupCalc(m, "COMUM", lossC);
    const dt = groupCalc(m, "DAY_TRADE", lossD);
    const f = groupCalc(m, "FII", lossF);
    lossC = comum.lossAfter;
    lossD = dt.lossAfter;
    lossF = f.lossAfter;

    const irrfDt = n(m?.irrf_dt);
    const irrfFii = n(m?.irrf_fii);
    const irrfLei = cents(n(m?.irrf_month) - irrfDt - irrfFii);

    // Operações comuns e day trade: um imposto só, abatido pelos dois IR na fonte.
    const totalTax = cents(comum.tax + dt.tax);
    const [usedDt, leftDt] = use(dtCarry + irrfDt, totalTax);
    const [usedLei, leftLei] = use(leiCarry + irrfLei, cents(totalTax - usedDt));
    const toPay = cents(totalTax - usedDt - usedLei);

    // FII: tela própria, com o seu IR na fonte.
    const [usedF, leftF] = use(fiiCarry + irrfFii, f.tax);
    const fiiToPay = cents(f.tax - usedF);

    // Um DARF só para tudo: a parte paga vai primeiro para as operações comuns.
    const paidTotal = n(m?.paid?.amount);
    const paidC = cents(Math.min(paidTotal, toPay));
    const paidF = cents(paidTotal - paidC);

    comuns.push({
      month: i,
      hasSales: sales.some((s) => s.group !== "FII"),
      comum,
      dt,
      totalTax,
      irrfDtMonth: irrfDt,
      irrfDtBefore: dtCarry,
      irrfDtComp: leftDt,
      irrfLeiMonth: irrfLei,
      irrfLeiBefore: leiCarry,
      irrfLeiComp: leftLei,
      toPay,
      paid: paidC,
    });
    fii.push({
      ...f,
      month: i,
      hasSales: sales.some((s) => s.group === "FII"),
      irrfBefore: fiiCarry,
      irrfMonth: irrfFii,
      irrfComp: leftF,
      toPay: fiiToPay,
      paid: paidF,
    });
    dtCarry = leftDt;
    leiCarry = leftLei;
    fiiCarry = leftF;
  }
  return { comuns, fii };
}

// ---------- rendimentos ----------

export type Payer = { asset: string; name: string; value: number };
export type IncomeCode = {
  id: string;
  label: string;
  /** Código que não temos certeza: o contador confirma. */
  confirm?: boolean;
  total: number;
  payers: Payer[];
  /** Só o total vai para o programa (sem fonte pagadora). */
  totalOnly?: boolean;
};

function payersOf(
  rows: { asset: string; value: number }[],
  names: Record<string, string>,
): Payer[] {
  const by = new Map<string, number>();
  for (const r of rows) by.set(r.asset, cents((by.get(r.asset) ?? 0) + r.value));
  return [...by.entries()]
    .map(([asset, value]) => ({ asset, name: names[asset] ?? "", value }))
    .filter((p) => p.value !== 0)
    .sort((a, b) => a.asset.localeCompare(b.asset));
}

function code(
  id: string,
  label: string,
  payers: Payer[],
  extra: Partial<IncomeCode> = {},
): IncomeCode {
  return {
    id,
    label,
    payers,
    total: cents(payers.reduce((s, p) => s + p.value, 0)),
    ...extra,
  };
}

/** Proventos do Brasil no ano, um item por fonte pagadora (o ativo). */
function incomesOf(rows: IncomeRow[], year: number, pick: (r: IncomeRow) => boolean) {
  return rows
    .filter(
      (r) =>
        r.income.pay_date.startsWith(String(year)) &&
        r.income.currency !== "USD" &&
        r.net_brl != null &&
        pick(r),
    )
    .map((r) => ({ asset: r.income.asset, value: n(r.net_brl) }));
}

export function isentos(
  rows: IncomeRow[],
  events: EventRow[],
  months: TaxMonth[],
  names: Record<string, string>,
  year: number,
): IncomeCode[] {
  const k = (r: IncomeRow) => r.income.kind;
  const dividendos = payersOf(
    incomesOf(rows, year, (r) => !["JCP", "JUROS", "RENDIMENTO"].includes(k(r))),
    names,
  );
  const bonus = payersOf(
    events
      .filter(
        (e) =>
          e.event.kind === "BONIFICACAO" &&
          e.ex_date.startsWith(String(year)) &&
          e.exempt_brl != null,
      )
      .map((e) => ({ asset: e.event.asset, value: n(e.exempt_brl) })),
    names,
  );
  const exempt = cents(
    months
      .filter((m) => m.month.startsWith(String(year)))
      .reduce((s, m) => s + n(m.exempt_profit), 0),
  );
  const fii = payersOf(incomesOf(rows, year, (r) => k(r) === "RENDIMENTO"), names);
  const out: IncomeCode[] = [];
  if (dividendos.length) out.push(code("09", "09. Lucros e dividendos recebidos", dividendos));
  if (bonus.length)
    out.push(
      code("18", "18. Incorporação de reservas ao capital / Bonificações em ações", bonus),
    );
  if (exempt > 0)
    out.push({
      id: "20",
      label:
        "20. Ganhos líquidos em operações no mercado à vista de ações negociadas em bolsas de valores nas alienações realizadas até R$ 20.000,00 em cada mês, para o total das ações",
      total: exempt,
      payers: [],
      totalOnly: true,
    });
  if (fii.length)
    out.push(code("99", "99. Outros: rendimentos de FII e Fiagro", fii, { confirm: true }));
  return out;
}

export function exclusivos(
  rows: IncomeRow[],
  names: Record<string, string>,
  year: number,
): IncomeCode[] {
  const jcp = payersOf(incomesOf(rows, year, (r) => r.income.kind === "JCP"), names);
  const juros = payersOf(incomesOf(rows, year, (r) => r.income.kind === "JUROS"), names);
  const out: IncomeCode[] = [];
  if (jcp.length) out.push(code("10", "10. Juros sobre capital próprio", jcp));
  if (juros.length)
    out.push(code("06", "06. Rendimentos de aplicações financeiras (juros)", juros, { confirm: true }));
  return out;
}

// ---------- bens e direitos ----------

export type Bem = {
  key: string;
  group: string | null;
  code: string | null;
  discriminacao: string;
  before: number;
  after: number;
  /** Ativo negociado na B3: o programa pede o código de negociação. */
  ticker: string | null;
  /** País, quando o bem está no exterior. */
  country: string | null;
  /** Lucro ou prejuízo das vendas no ano (Lei 14.754/2023). */
  profit: number | null;
};

/** País do ativo no exterior: os ETFs de Londres (UCITS) são fundos da Irlanda. */
export function countryOf(asset: string): string {
  return /\.L$/.test(asset) ? "375 - Irlanda" : "249 - Estados Unidos";
}

const B3_KINDS = ["ACAO", "FII", "ETF", "ETF_RF", "BDR", "FI_INFRA"];

export function bens(
  view: TaxView,
  kinds: Record<string, string>,
  names: Record<string, string>,
  year: number,
): Bem[] {
  const now = view.annual.positions[String(year)] ?? [];
  const prev = view.annual.positions[String(year - 1)] ?? [];
  const foreign = view.annual.foreign.find((y) => y.year === year);
  const profit = new Map<string, number>();
  for (const s of foreign?.sales ?? [])
    profit.set(s.asset, cents((profit.get(s.asset) ?? 0) + n(s.result_brl)));

  const assets = new Map<string, { now?: YearEndPosition; prev?: YearEndPosition }>();
  for (const p of prev) assets.set(p.asset, { prev: p });
  for (const p of now) assets.set(p.asset, { ...assets.get(p.asset), now: p });
  for (const a of profit.keys()) if (!assets.has(a)) assets.set(a, {});

  const out: Bem[] = [];
  for (const [asset, { now: a, prev: b }] of [...assets.entries()].sort((x, y) =>
    x[0].localeCompare(y[0]),
  )) {
    const kind = kinds[asset];
    const c = bensCode(asset, kind);
    const ref = a ?? b;
    const usd = ref?.cost_usd != null || profit.has(asset);
    out.push({
      key: asset,
      group: c?.group ?? null,
      code: c?.code ?? null,
      discriminacao: ref
        ? discriminacao(ref, kind, names[asset])
        : `${asset}${names[asset] && names[asset] !== asset ? ` (${names[asset]})` : ""}, vendido no ano`,
      before: n(b?.cost_brl),
      after: n(a?.cost_brl),
      ticker: !usd && B3_KINDS.includes(kind ?? "") ? asset : null,
      country: usd && kind !== "CRIPTO" ? countryOf(asset) : null,
      profit: profit.has(asset) ? profit.get(asset)! : usd && kind !== "CRIPTO" ? 0 : null,
    });
  }

  const bal = (y: number) =>
    // FGTS e cartão benefício não precisam ir para a declaração.
    view.savings_year_end.filter((x) => x.year === y && x.kind !== "Cartão benefício" && x.kind !== "FGTS");
  const id = (x: { kind: string; name: string; account: string }) =>
    `${x.kind}|${x.name}|${x.account}`;
  const balNow = bal(year);
  const balPrev = new Map(bal(year - 1).map((x) => [id(x), x]));
  const seen = new Set<string>();
  for (const x of [...balNow, ...balPrev.values()]) {
    const k = id(x);
    if (seen.has(k)) continue;
    seen.add(k);
    const cur = balNow.find((y) => id(y) === k);
    const old = balPrev.get(k);
    const conta = x.kind === "Conta corrente";
    out.push({
      key: k,
      group: conta ? "06" : null,
      code: conta ? "01" : null,
      discriminacao: `${x.kind}${x.name ? `: ${x.name}` : ""}${x.account ? ` · ${x.account}` : ""}`,
      before: n(old?.value),
      after: n(cur?.value),
      ticker: null,
      country: x.usd != null ? "249 - Estados Unidos" : null,
      profit: null,
    });
  }
  return out;
}
