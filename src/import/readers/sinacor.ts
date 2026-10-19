// Notas de corretagem da B3 no padrão SINACOR (Itaú, Nubank, XP, Clear, Rico,
// BTG, Inter e outras). Cada nota tem cabeçalho com número e data do pregão,
// a lista de negócios e os resumos de negócios e financeiro.

import { allocate, brDate, brNumber, fromCents, grossCents, toCents } from "../numbers";
import type { NoteFee, NoteLine, ParsedNote, Reader } from "../types";

const BROKERS: [RegExp, string][] = [
  [/Ita[uú]\s*Corretora/i, "Itaú"],
  [/Nu Investimentos/i, "Nubank"],
  [/XP Investimentos/i, "XP"],
  [/Clear Corretora/i, "Clear"],
  [/Rico Investimentos/i, "Rico"],
  [/BTG Pactual/i, "BTG Pactual"],
  [/Inter (?:Distribuidora|DTVM)/i, "Inter"],
  [/Genial Investimentos/i, "Genial"],
  [/Ágora|Agora Corretora/i, "Ágora"],
  [/Toro (?:Corretora|Investimentos)/i, "Toro"],
  [/Modal/i, "Modal"],
];

const MARKETS = [
  "VISTA",
  "FRACIONARIO",
  "TERMO",
  "OPCAO DE COMPRA",
  "OPCAO DE VENDA",
  "EXERC OPC COMPRA",
  "EXERC OPC VENDA",
  "LEILAO",
];
const SUPPORTED = new Set(["VISTA", "FRACIONARIO"]);

/** Classes que aparecem depois do nome de pregão. */
const CLASSES = /^(ON|PN|PNA|PNB|PNC|PND|UNT|CI|DR1|DR2|DR3)$/;
const OBS = /^[@#DFCHPXYTLIBA28]{1,5}$/;

/** Uma linha de negócio, já sem as repetições do Itaú ("CompraCompraC"). */
const OP = new RegExp(
  "^(?:\\d-)?(?:BOVESPA|B3 RV LISTADO|B3|BMF)\\S*\\s+(C|V)\\s+(" +
    MARKETS.join("|") +
    ")\\s+(.+?)\\s+(\\d[\\d.]*)\\s+(?:R\\$\\s*)?(\\d[\\d.]*,\\d+)\\s+(?:R\\$\\s*)?(\\d[\\d.]*,\\d{2})\\s*(D|C)$",
);

function clean(line: string): string {
  return line
    .replace(/(?:Compra)+C\b/g, "C")
    .replace(/(?:Venda)+V\b/g, "V")
    .replace(/(?:Débito)+D$/, "D")
    .replace(/(?:Crédito)+C$/, "C")
    .trim();
}

function brokerName(text: string): string {
  for (const [re, name] of BROKERS) if (re.test(text)) return name;
  const first = text.split("\n").find((l) => /Corretora|Investimentos|DTVM|CTVM/i.test(l));
  return first ? first.split(/\s{2,}| - /)[0].trim() : "Corretora";
}

/** Valor em reais logo depois do rótulo, na primeira vez que aparece. */
function amount(text: string, label: string): { value: string; debit: boolean } | null {
  const re = new RegExp(label + "[ \\t]+(-?)(?:R\\$)?[ \\t]*(-?[\\d.]+,\\d{2})([ \\t]*(?:D[ée]bito|D\\b))?", "i");
  const m = re.exec(text);
  if (!m) return null;
  const v = brNumber(m[2]);
  if (v == null) return null;
  return { value: v.replace("-", ""), debit: m[1] === "-" || m[2].startsWith("-") || !!m[3] };
}

const FEE_LABELS: [string, string][] = [
  ["Taxa de liquida[çc][ãa]o(?:/CCP)?", "Taxa de liquidação"],
  ["Taxa de registro", "Taxa de registro"],
  ["Taxa de termo\\s*/\\s*op[çc][õo]es", "Taxa de termo/opções"],
  ["Taxa A\\.N\\.A\\.", "Taxa A.N.A."],
  ["Emolumentos", "Emolumentos"],
  ["Taxa de transf(?:er[êe]ncia|\\.)\\s*de ativos", "Taxa de transferência de ativos"],
  ["(?<!Total )Corretagem", "Corretagem"],
  ["(?<!Total )Clearing", "Clearing"],
  ["Execu[çc][ãa]o", "Execução"],
  ["Execu[çc][ãa]o casa", "Execução casa"],
  ["ISS\\s*\\(?\\s*[A-ZÃÇÉ ]*\\)?", "ISS"],
  ["Outras(?: taxas)?", "Outras"],
];

function header(lines: string[]): { date: string | null; number: string | null; sheet: [number, number] | null } {
  let date: string | null = null;
  let number: string | null = null;
  let sheet: [number, number] | null = null;
  for (let i = 0; i < lines.length && i < 40; i++) {
    const l = lines[i];
    if (!date && /Data\s+(?:do\s+)?preg[ãa]o/i.test(l)) {
      const same = /Data\s+(?:do\s+)?preg[ãa]o\s+(\d{2}\/\d{2}\/\d{4})/i.exec(l);
      const near = same?.[1] ?? [lines[i + 1], lines[i + 2]].join(" ").match(/\d{2}\/\d{2}\/\d{4}/)?.[0];
      if (near) date = brDate(near);
      const sh = /(\d+)\s*\/\s*(\d+)\s*$/.exec(lines[i + 1] ?? "");
      if (sh) sheet = [Number(sh[1]), Number(sh[2])];
    }
    if (!number && /N(?:º|o\.?|r\.?|úmero|umero) (?:da )?nota/i.test(l)) {
      const same = /N(?:º|o\.?|r\.?|úmero|umero) (?:da )?nota\s+(\d{3,})/i.exec(l);
      number = same?.[1] ?? (lines[i + 1] ?? "").split(/\s+/).find((t) => /^\d{3,}$/.test(t)) ?? null;
    }
  }
  return { date, number, sheet };
}

function parseLine(raw: string): NoteLine | "unsupported" | null {
  const m = OP.exec(clean(raw));
  if (!m) return null;
  const [, cv, market, rest, qty, price, value] = m;
  const parts = rest.split(/\s{2,}/);
  let obs = "";
  if (parts.length > 1 && OBS.test(parts[parts.length - 1])) obs = parts.pop() as string;
  const title = parts.join(" ").replace(/\s+/g, " ").trim();
  const tokens = title.split(" ");
  let ticker: string | null = null;
  let lookup: NoteLine["lookup"];
  // O código costuma vir no início ("ABCD3 ON NM"), mas a XP o põe depois do
  // nome de pregão ("FII EXEMPLO ABCD11 CI").
  const code = tokens.find((t) => /^[A-Z]{4}\d{1,2}F?$/.test(t));
  if (code) {
    ticker = code.replace(/^([A-Z]{4}\d{1,2})F$/, "$1");
  } else {
    const k = tokens.findIndex((t) => CLASSES.test(t));
    if (k > 0) lookup = { name: tokens.slice(0, k).join(" "), cls: tokens[k] };
  }
  const line: NoteLine = {
    side: cv === "C" ? "COMPRA" : "VENDA",
    ticker,
    lookup,
    title,
    market,
    quantity: brNumber(qty) ?? qty,
    price: brNumber(price) ?? price,
    value: brNumber(value) ?? value,
    fees: "0",
    dayTrade: obs.includes("D"),
  };
  if (!SUPPORTED.has(market)) line.skip = `Mercado ${market.toLowerCase()} ainda não é acompanhado pelo aplicativo.`;
  return line;
}

function readNote(pages: string[][]): ParsedNote {
  const all = pages.flat();
  const text = all.join("\n");
  const { date, number } = header(pages[0]);
  const warnings: string[] = [];
  const lines: NoteLine[] = [];
  for (const l of all) {
    const p = parseLine(l);
    if (p && p !== "unsupported") lines.push(p);
  }

  const fees: NoteFee[] = [];
  let feeCents = 0n;
  for (const [re, label] of FEE_LABELS) {
    const a = amount(text, re);
    if (a && toCents(a.value) !== 0n) {
      fees.push({ label, amount: a.value });
      feeCents += toCents(a.value);
    }
  }
  const irrfM = /I\.?R\.?R\.?F\.?\s*s\/\s*opera[çc][õo]es[.,]?\s*base\s*(?:R\$)?\s*[\d.,]+\s+(?:R\$)?\s*([\d.,]+)/i.exec(text);
  const irrf = irrfM ? toCents(brNumber(irrfM[1]) ?? "0") : 0n;
  if (irrf > 0n) warnings.push(`IRRF retido na nota ("dedo-duro"): R$ ${fromCents(irrf).replace(".", ",")}.`);
  const dtM = /IRRF Day[- ]Trade:?\s*Base\s*(?:R\$)?\s*[\d.,]+\s+Proje[çc][ãa]o:?\s*(?:R\$)?\s*([\d.,]+)/i.exec(text);
  const irrfDt = dtM ? toCents(brNumber(dtM[1]) ?? "0") : 0n;
  if (irrfDt > 0n) warnings.push(`IRRF de day trade na nota: R$ ${fromCents(irrfDt).replace(".", ",")}.`);
  if (lines.some((l) => l.dayTrade))
    warnings.push("A nota tem day trade (marca D). O aplicativo ainda não separa a apuração de day trade.");

  // Rateio das taxas pelo valor de cada linha.
  const weights = lines.map((l) => toCents(l.value));
  allocate(feeCents, weights).forEach((c, i) => (lines[i].fees = fromCents(c)));

  // Conferência: soma das linhas, valor das operações e líquido da nota.
  const problems: string[] = [];
  for (const l of lines) {
    if (grossCents(l.quantity, l.price) !== toCents(l.value))
      problems.push(`${l.title}: ${l.quantity} × ${l.price} não dá ${l.value}`);
  }
  const buys = lines.filter((l) => l.side === "COMPRA").reduce((a, l) => a + toCents(l.value), 0n);
  const sells = lines.filter((l) => l.side === "VENDA").reduce((a, l) => a + toCents(l.value), 0n);
  const total = amount(text, "Valor das opera[çc][õo]es");
  if (total && toCents(total.value) !== buys + sells)
    problems.push(
      `soma das linhas R$ ${fromCents(buys + sells)} diferente do valor das operações R$ ${total.value}`,
    );
  const net = /L[íi]quido para\s+(\d{2}\/\d{2}\/\d{4})\s+(-?)(?:R\$)?\s*([\d.,]+)(\s*(?:D[ée]bito|D\b))?/i.exec(text);
  if (net) {
    const signed = (net[2] === "-" || net[4] ? -1n : 1n) * toCents(brNumber(net[3]) ?? "0");
    const expected = sells - buys - feeCents - irrf - irrfDt;
    if (signed !== expected)
      problems.push(
        `líquido da nota R$ ${fromCents(signed)} diferente do calculado R$ ${fromCents(expected)} (operações − taxas − IRRF)`,
      );
  } else problems.push("não achei o líquido da nota");
  if (!date) problems.push("não achei a data do pregão");
  if (lines.length === 0) problems.push("não achei nenhum negócio");

  const broker = brokerName(text);
  return {
    reader: "sinacor",
    broker,
    currency: "BRL",
    number,
    date: date ?? "",
    lines,
    fees,
    totalFees: fromCents(feeCents),
    check: problems.length
      ? { ok: false, text: problems.join("; ") }
      : { ok: true, text: "Totais da nota conferidos: linhas, valor das operações, taxas e líquido batem." },
    warnings,
    identity: number && date ? `sinacor|${broker}|${number}|${date}` : text,
  };
}

export const sinacor: Reader = {
  id: "sinacor",
  detect(pages) {
    const t = pages.flat().join("\n");
    let score = 0;
    if (/Nota de (?:corretagem|negocia[çc][ãa]o)|Neg[óo]cios realizados|Resumo dos Neg[óo]cios/i.test(t)) score += 0.4;
    if (/Data\s+(?:do\s+)?preg[ãa]o/i.test(t)) score += 0.3;
    if (/L[íi]quido para\s+\d{2}\/\d{2}\/\d{4}/i.test(t)) score += 0.3;
    return score;
  },
  read(pages) {
    // Páginas seguidas com o mesmo número e data formam uma nota.
    const groups: { key: string; pages: string[][] }[] = [];
    for (const page of pages) {
      const h = header(page);
      const key = `${h.number ?? "?"}|${h.date ?? "?"}`;
      const last = groups[groups.length - 1];
      const continues = last && (last.key === key || (h.sheet && h.sheet[0] > 1 && !h.number));
      if (continues) last.pages.push(page);
      else if (h.date || h.number) groups.push({ key, pages: [page] });
      else if (last) last.pages.push(page);
    }
    return groups.map((g) => readNote(g.pages));
  },
};
