// Confirmação de negociação da Interactive Brokers ("Relatório de confirmação
// de negociação" / "Trade Confirmation Report"), em português ou inglês.
//
// Cada ordem vem numa linha com o mercado "-" e, logo abaixo, uma linha por
// execução (com a bolsa em que executou); quando a ordem executa de uma vez,
// pode vir só a linha da execução. O app usa a linha da ordem quando existe.
// Depois vêm os totais por ativo ("Total CSPX (Comprado)"), usados na
// conferência, e a tabela dos instrumentos, que diz a bolsa de cada código
// (LSEETF = Bolsa de Londres, que o app guarda como CSPX.L).
//
// A corretagem às vezes vem 0,00 na confirmação e é cobrada à parte, no
// extrato do dia; nesse caso o app avisa e deixa digitar a taxa.

import { fromCents, grossCents, toCents, usNumber } from "../numbers";
import type { NoteLine, ParsedNote, Reader } from "../types";

const NUM = String.raw`-?[\d.,]+`;
const ROW = new RegExp(
  String.raw`^(\S+)\s{2,}(\S+)\s{2,}(\d{4}-\d{2}-\d{2}),\s*([\d:]+)\s{2,}(\d{4}-\d{2}-\d{2})\s{2,}(\S+)\s{2,}(BUY|SELL)\s{2,}` +
    `(${NUM})\\s{2,}(${NUM})\\s{2,}(${NUM})\\s{2,}(${NUM})\\s{2,}(${NUM})(?:\\s{2,}.*)?$`,
);
const TOTAL = new RegExp(String.raw`^Total\s+(\S+)\s+\(([^)]+)\)\s{2,}(${NUM})\s{2,}(${NUM})\s{2,}(${NUM})\s{2,}(${NUM})\s{2,}(${NUM})$`);
const INSTRUMENT = /^(\S+)\s{2,}(.+?)\s{2,}(\d+)\s{2,}([A-Z]{2}[A-Z0-9]{9}\d)\s{2,}(\S+)\s{2,}(\S+)\s{2,}(\d+)\s{2,}(\S+)/;
const CURRENCY = /^(USD|EUR|GBP|GBX|CHF|CAD|JPY|HKD|AUD)$/;
const US_EXCHANGES = new Set(["NASDAQ", "NYSE", "ARCA", "AMEX", "BATS", "NYSEARCA", "NYSEAMEX", "IEX", "PINK"]);

/** "-21,387.46" → centavos com sinal. */
function cents(raw: string): bigint {
  return toCents(usNumber(raw) ?? "0");
}

function abs(c: bigint): bigint {
  return c < 0n ? -c : c;
}

type Instrument = { name: string; listing: string; isin: string };

/** Código do ativo no app e o motivo para não importar, pela bolsa de listagem. */
export function ibkrTicker(symbol: string, inst: Instrument | undefined): { ticker: string; skip?: string } {
  const listing = inst?.listing.toUpperCase() ?? "";
  if (listing.startsWith("LSE")) return { ticker: `${symbol}.L` };
  if (!inst || US_EXCHANGES.has(listing) || inst.isin.startsWith("US")) return { ticker: symbol };
  return {
    ticker: symbol,
    skip: `Ativo listado na bolsa ${inst.listing}: o app acompanha só os EUA e a Bolsa de Londres. Lance à mão se quiser registrar.`,
  };
}

type Row = {
  account: string;
  symbol: string;
  date: string;
  time: string;
  side: "COMPRA" | "VENDA";
  quantity: string;
  price: string;
  proceeds: bigint;
  fees: bigint;
  currency: string;
  raw: string;
};

export const ibkr: Reader = {
  id: "ibkr",
  detect(pages) {
    const t = pages.flat().join("\n");
    return /Interactive Brokers/.test(t) && /(Relatório de confirmação de negociação|Trade Confirmation Report)/i.test(t) ? 0.95 : 0;
  },
  read(pages) {
    const lines = pages.flat();
    const instruments = new Map<string, Instrument>();
    for (const l of lines) {
      const m = INSTRUMENT.exec(l);
      if (m) instruments.set(m[1], { name: m[2].trim(), isin: m[4], listing: m[6] });
    }

    const rows: Row[] = [];
    const totals: { symbol: string; quantity: string; proceeds: bigint; fees: bigint }[] = [];
    let currency = "USD";
    let inOrder = false;
    for (const l of lines) {
      const cur = CURRENCY.exec(l.trim());
      if (cur) {
        currency = cur[1];
        inOrder = false;
        continue;
      }
      const t = TOTAL.exec(l);
      if (t) {
        inOrder = false;
        totals.push({ symbol: t[1], quantity: usNumber(t[3]) ?? t[3], proceeds: cents(t[5]), fees: abs(cents(t[6])) + abs(cents(t[7])) });
        continue;
      }
      const m = ROW.exec(l);
      if (!m) continue;
      const [, account, symbol, date, time, , market, type, qty, price, proceeds, comm, fee] = m;
      const isOrder = market === "-";
      // Execuções logo abaixo da linha da ordem já estão somadas nela.
      if (!isOrder && inOrder) continue;
      inOrder = isOrder;
      rows.push({
        account,
        symbol,
        date,
        time,
        side: type === "BUY" ? "COMPRA" : "VENDA",
        quantity: (usNumber(qty) ?? qty).replace("-", ""),
        price: usNumber(price) ?? price,
        proceeds: cents(proceeds),
        fees: abs(cents(comm)) + abs(cents(fee)),
        currency,
        raw: [account, symbol, date, time, type, qty, price].join(" "),
      });
    }

    const dates = [...new Set(rows.map((r) => r.date))].sort();
    return dates.map((date): ParsedNote => {
      const rs = rows.filter((r) => r.date === date);
      const problems: string[] = [];
      const lines: NoteLine[] = rs.map((r) => {
        const inst = instruments.get(r.symbol);
        const { ticker, skip } = ibkrTicker(r.symbol, inst);
        const value = abs(r.proceeds);
        const gross = grossCents(r.quantity, r.price);
        if (gross - value > 2n || value - gross > 2n) problems.push(`${r.symbol}: ${r.quantity} × ${r.price} não dá ${fromCents(value)}`);
        const line: NoteLine = {
          side: r.side,
          ticker,
          title: inst ? `${r.symbol} · ${inst.name}` : r.symbol,
          market: inst?.listing.toUpperCase().startsWith("LSE") ? "Londres" : "EUA",
          quantity: r.quantity,
          price: r.price,
          value: fromCents(value),
          fees: fromCents(r.fees),
          dayTrade: false,
        };
        if (r.currency !== "USD") line.skip = `Negociado em ${r.currency}: o app registra só operações em dólar.`;
        else if (skip) line.skip = skip;
        return line;
      });
      // Totais por ativo: só confere quando o documento tem um único dia.
      if (dates.length === 1) {
        for (const t of totals) {
          const mine = rs.filter((r) => r.symbol === t.symbol);
          if (!mine.length) continue;
          const sum = mine.reduce((a, r) => a + r.proceeds, 0n);
          if (sum !== t.proceeds) problems.push(`${t.symbol}: as ordens somam ${fromCents(sum)}, mas o total diz ${fromCents(t.proceeds)}`);
        }
      }
      const feeCents = rs.reduce((a, r) => a + r.fees, 0n);
      const warnings: string[] = [];
      if (rs.some((r) => r.fees === 0n))
        warnings.push(
          "A corretagem veio zerada nesta confirmação. A IBKR às vezes cobra à parte, no extrato do dia: confira no extrato ou no app da IBKR e digite a taxa de cada ordem antes de importar.",
        );
      return {
        reader: "ibkr",
        broker: "Interactive Brokers",
        currency: "USD",
        number: null,
        date,
        lines,
        fees: feeCents > 0n ? [{ label: "Corretagem e taxas", amount: fromCents(feeCents) }] : [],
        totalFees: fromCents(feeCents),
        feesEditable: true,
        check: problems.length
          ? { ok: false, text: problems.join("; ") }
          : { ok: true, text: "Totais conferidos: quantidade × preço de cada ordem e o total de cada ativo batem." },
        warnings,
        // Sem a corretagem: a mesma confirmação gerada de novo, já com ela, é a mesma nota.
        identity: ["ibkr", date, ...rs.map((r) => r.raw)].join("\n"),
      };
    });
  },
};
