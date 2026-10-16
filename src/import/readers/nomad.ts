// Confirmações de negociação da Nomad (exterior, em dólar), nos dois layouts:
// - antigo, da DriveWealth ("Transaction Confirmation"): cada ordem pode vir
//   em duas linhas, a parte inteira (Agency) e a fração (Principal), com o
//   mesmo horário e preço; as duas viram uma operação;
// - atual, da Apex Clearing ("Trade Confirmation"): seções "You Bought" e
//   "You Sold", uma linha por ordem.

import { fromCents, grossCents, toCents, usDate, usNumber } from "../numbers";
import type { NoteLine, ParsedNote, Reader } from "../types";

const CUSTODY_RESIDUE =
  "Venda de fração mínima com valor zero: resíduo da troca de custodiante (DriveWealth → Apex). " +
  "Use a transferência de custódia em vez de importar como venda.";

/** "$1,234.56", "($0.01)" ou "-$0.01" → centavos com sinal. */
function usdCents(raw: string): bigint {
  const neg = /^\(.*\)$/.test(raw.trim()) || raw.trim().startsWith("-");
  const v = usNumber(raw.replace(/[()]/g, "").replace("-", "")) ?? "0";
  const c = toCents(v);
  return neg ? -c : c;
}

type Row = {
  side: "COMPRA" | "VENDA";
  symbol: string;
  title: string;
  date: string;
  key: string;
  quantity: string;
  price: string;
  principal: bigint;
  fees: bigint;
  net: bigint;
  raw: string[];
};

/** Soma decimais em texto com até 8 casas sem perder precisão. */
function addDec(a: string, b: string): string {
  const scale = 10;
  const toInt = (s: string) => {
    const neg = s.startsWith("-");
    const [i, f = ""] = s.replace("-", "").split(".");
    const v = BigInt(i || "0") * 10n ** BigInt(scale) + BigInt((f + "0".repeat(scale)).slice(0, scale));
    return neg ? -v : v;
  };
  const sum = toInt(a) + toInt(b);
  const neg = sum < 0n;
  const abs = neg ? -sum : sum;
  const int = abs / 10n ** BigInt(scale);
  const frac = String(abs % 10n ** BigInt(scale)).padStart(scale, "0").replace(/0+$/, "");
  return `${neg ? "-" : ""}${int}${frac ? "." + frac : ""}`;
}

/** Junta linhas da mesma ordem e monta uma nota por data de pregão. */
function toNotes(reader: string, rows: Row[], header: string): ParsedNote[] {
  const merged: Row[] = [];
  for (const r of rows) {
    const same = merged.find((m) => m.key === r.key);
    if (same) {
      same.quantity = addDec(same.quantity, r.quantity);
      same.principal += r.principal;
      same.fees += r.fees;
      same.net += r.net;
      same.raw.push(...r.raw);
    } else merged.push({ ...r, raw: [...r.raw] });
  }
  const dates = [...new Set(merged.map((r) => r.date))].sort();
  return dates.map((date) => {
    const rs = merged.filter((r) => r.date === date);
    const problems: string[] = [];
    const lines: NoteLine[] = rs.map((r) => {
      const qty = r.quantity.replace("-", "");
      const line: NoteLine = {
        side: r.side,
        ticker: r.symbol,
        title: r.title ? `${r.symbol} · ${r.title}` : r.symbol,
        market: "EUA",
        quantity: qty,
        price: r.price,
        value: fromCents(r.principal < 0n ? -r.principal : r.principal),
        fees: fromCents(r.fees > 0n ? r.fees : 0n),
        dayTrade: false,
      };
      if (r.side === "VENDA" && r.principal === 0n) line.skip = CUSTODY_RESIDUE;
      const expected = r.side === "COMPRA" ? r.principal + r.fees : r.principal - r.fees;
      if (expected !== r.net) problems.push(`${r.symbol}: valor ${fromCents(r.principal)} e taxas não dão o líquido ${fromCents(r.net)}`);
      const gross = grossCents(qty, r.price);
      const diff = gross - r.principal;
      if (r.principal !== 0n && (diff > 2n || diff < -2n))
        problems.push(`${r.symbol}: ${qty} × ${r.price} não dá ${fromCents(r.principal)}`);
      return line;
    });
    const feeCents = rs.reduce((a, r) => a + (r.fees > 0n ? r.fees : 0n), 0n);
    const warnings: string[] = [];
    if (rs.some((r) => r.fees < 0n))
      warnings.push("A confirmação tem créditos de arredondamento (\"Other Fees / Credits\"); eles foram ignorados.");
    if (lines.some((l) => l.skip === CUSTODY_RESIDUE))
      warnings.push("Há vendas de valor zero da troca de custodiante; elas vêm desmarcadas.");
    return {
      reader,
      broker: "Nomad",
      currency: "USD",
      number: null,
      date,
      lines,
      fees: feeCents > 0n ? [{ label: "Comissões e taxas", amount: fromCents(feeCents) }] : [],
      totalFees: fromCents(feeCents),
      check: problems.length
        ? { ok: false, text: problems.join("; ") }
        : { ok: true, text: "Totais conferidos: quantidade × preço, taxas e valor líquido de cada ordem batem." },
      warnings,
      identity: [reader, header, date, ...rs.flatMap((r) => r.raw)].join("\n"),
    };
  });
}

// ---------- DriveWealth ----------

const DW_ROW =
  /^([A-Z][A-Z.\-]*)\s{2,}(.+?)\s{2,}\S+\s{2,}(Buy|Sell)\s{2,}(\d{1,2}:\d{2}:\d{2}\s*[AP]M)\s{2,}(-?[\d.,]+)\s{2,}([\d.,]+)\s{2,}(\d{1,2}\/\d{1,2}\/\d{4})\s{2,}(\d{1,2}\/\d{1,2}\/\d{4})\s{2,}(.+)$/;

function dwValue(lines: string[], from: number, label: string): bigint {
  for (let i = from; i < Math.min(lines.length, from + 8); i++) {
    if (i > from && DW_ROW.test(lines[i])) break;
    const m = new RegExp("^" + label + "\\s+(\\(?-?\\$?[\\d.,]+\\)?)").exec(lines[i]);
    if (m) return usdCents(m[1]);
  }
  return 0n;
}

export const nomadDriveWealth: Reader = {
  id: "nomad-drivewealth",
  detect(pages) {
    const t = pages.flat().join("\n");
    return /DriveWealth/.test(t) && /Transaction/.test(t) && /Confirmation/.test(t) ? 0.95 : 0;
  },
  read(pages) {
    const lines = pages.flat();
    const header = lines.find((l) => /Confirmation Date/.test(l)) ?? "";
    const rows: Row[] = [];
    lines.forEach((l, i) => {
      const m = DW_ROW.exec(l);
      if (!m) return;
      const [, symbol, title, action, time, qty, price, trade] = m;
      const date = usDate(trade) ?? "";
      const principal = dwValue(lines, i, "Principal Amount");
      const fees = dwValue(lines, i, "Commission") + dwValue(lines, i, "Transaction Fee") + dwValue(lines, i, "Other Fees / Credits");
      const net = dwValue(lines, i, "Net Amount");
      const p = usNumber(price) ?? price;
      rows.push({
        side: action === "Buy" ? "COMPRA" : "VENDA",
        symbol,
        title: title.trim(),
        date,
        key: `${symbol}|${action}|${time}|${p}|${date}`,
        quantity: usNumber(qty) ?? qty,
        price: p,
        principal,
        fees,
        net,
        raw: [l],
      });
    });
    return toNotes("nomad-drivewealth", rows, header);
  },
};

// ---------- Apex ----------

const APEX_ROW =
  /^(\d{4}-\d{2}-\d{2})\s+(\d{4}-\d{2}-\d{2})\s+([A-Z][A-Z.\-]*)\s+(-?[\d.,]+)\s+([\d.,]+)\s+(-?[\d.,]+)\s+(-?[\d.,]+)\s+(-?[\d.,]+)\s+(-?[\d.,]+)\s+(-?[\d.,]+)$/;

export const nomadApex: Reader = {
  id: "nomad-apex",
  detect(pages) {
    const t = pages.flat().join("\n");
    return /Trade Confirmation/.test(t) && /You (?:Bought|Sold)/.test(t) ? 0.95 : 0;
  },
  read(pages) {
    const lines = pages.flat();
    const header = lines.find((l) => /Process Date/.test(l)) ?? "";
    const rows: Row[] = [];
    let side: Row["side"] | null = null;
    lines.forEach((l, i) => {
      if (/^You Bought/.test(l)) side = "COMPRA";
      else if (/^You Sold/.test(l)) side = "VENDA";
      const m = APEX_ROW.exec(l);
      if (!m || !side) return;
      const [, trade, , symbol, qty, price, gross, comm, fee, addl, net] = m;
      const desc = lines[i + 1] ?? "";
      const title = /DESC:\s*(.+?)(?:\s{2,}|$)/.exec(desc)?.[1] ?? "";
      const tradeNo = /Trade#:\s*(\S+)/.exec(desc)?.[1] ?? String(i);
      rows.push({
        side,
        symbol,
        title,
        date: trade,
        key: tradeNo,
        quantity: usNumber(qty) ?? qty,
        price: usNumber(price) ?? price,
        principal: usdCents(gross),
        fees: usdCents(comm) + usdCents(fee) + usdCents(addl),
        net: usdCents(net),
        raw: [l, desc],
      });
    });
    return toNotes("nomad-apex", rows, header);
  },
};
