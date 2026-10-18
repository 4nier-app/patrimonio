// Planilha (CSV) com lançamentos, para corretoras sem leitor de nota e para
// históricos guardados em planilha. Uma linha por compra ou venda.
//
// Colunas (pelo nome do cabeçalho, sem diferenciar maiúsculas e acentos):
//   data, conta, ativo, operacao, quantidade, preco   (obrigatórias)
//   taxas, moeda, ir, iof, observacao                 (opcionais)
// Separador ";" (Excel em português), "," ou tabulação. Com ";", a vírgula é
// o decimal (1.234,56); com ",", o ponto (1234.56).

import { brNumber, usNumber } from "./numbers";

export type CsvRow = {
  /** Número da linha no arquivo (o cabeçalho é a 1). */
  line: number;
  date: string;
  account: string;
  asset: string;
  side: "COMPRA" | "VENDA";
  quantity: string;
  price: string;
  /** Vazio: taxas calculadas pelo aplicativo. */
  fees: string | null;
  currency: "BRL" | "USD" | null;
  ir: string | null;
  iof: string | null;
  note: string | null;
};

export type CsvError = { line: number; message: string };

export type CsvResult = { rows: CsvRow[]; errors: CsvError[] };

const plain = (s: string) =>
  s
    .replace(/^﻿/, "")
    .trim()
    .toLowerCase()
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .replace(/[^a-z0-9]/g, "");

const COLUMNS: Record<string, string[]> = {
  date: ["data", "date", "datapregao", "datadaoperacao", "dataoperacao"],
  account: ["conta", "corretora", "banco", "account", "broker"],
  asset: ["ativo", "codigo", "ticker", "papel", "asset", "symbol"],
  side: ["operacao", "tipo", "cv", "compravenda", "side"],
  quantity: ["quantidade", "qtd", "qtde", "quantity"],
  price: ["preco", "precounitario", "preco unitario", "valorunitario", "price"],
  fees: ["taxas", "taxa", "custos", "fees", "corretagem"],
  currency: ["moeda", "currency"],
  ir: ["ir", "irretido"],
  iof: ["iof"],
  note: ["observacao", "obs", "nota", "note"],
};

const REQUIRED = ["date", "account", "asset", "side", "quantity", "price"] as const;
const LABEL: Record<string, string> = {
  date: "data",
  account: "conta",
  asset: "ativo",
  side: "operação",
  quantity: "quantidade",
  price: "preço",
};

/** Divide uma linha respeitando aspas ("1.234,56" com o separador dentro). */
function split(line: string, sep: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quoted = false;
  for (let i = 0; i < line.length; i++) {
    const c = line[i];
    if (c === '"') {
      if (quoted && line[i + 1] === '"') {
        cur += '"';
        i++;
      } else quoted = !quoted;
    } else if (c === sep && !quoted) {
      out.push(cur);
      cur = "";
    } else cur += c;
  }
  out.push(cur);
  return out.map((s) => s.trim());
}

function separator(header: string): string {
  const count = (c: string) => header.split(c).length - 1;
  return [";", "\t", ","].reduce((best, c) => (count(c) > count(best) ? c : best), ";");
}

/** "02/03/2026", "2/3/2026" ou "2026-03-02" → "2026-03-02". */
function parseDate(s: string): string | null {
  let m = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(s);
  if (m) {
    const [d, mo, y] = [Number(m[1]), Number(m[2]), m[3]];
    if (mo < 1 || mo > 12 || d < 1 || d > 31) return null;
    return `${y}-${String(mo).padStart(2, "0")}-${String(d).padStart(2, "0")}`;
  }
  m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s);
  return m ? s : null;
}

function parseSide(s: string): "COMPRA" | "VENDA" | null {
  const p = plain(s);
  if (["c", "compra", "comprar", "buy", "b", "aplicacao"].includes(p)) return "COMPRA";
  if (["v", "venda", "vender", "sell", "s", "resgate"].includes(p)) return "VENDA";
  return null;
}

function parseNumber(s: string, sep: string): string | null {
  const t = s.replace(/R\$|US\$|\$|\s/g, "");
  if (!t) return null;
  // Com os dois sinais, o último é o decimal; com um só, decide o separador.
  if (t.includes(",") && t.includes(".")) return t.lastIndexOf(",") > t.lastIndexOf(".") ? brNumber(t) : usNumber(t);
  if (sep === ",") return usNumber(t);
  return brNumber(t);
}

export function parseCsv(text: string): CsvResult {
  const lines = text.split(/\r?\n/);
  const header = lines[0] ?? "";
  const sep = separator(header);
  const names = split(header, sep).map(plain);
  const col: Record<string, number> = {};
  for (const [key, aliases] of Object.entries(COLUMNS)) {
    const i = names.findIndex((n) => aliases.map(plain).includes(n));
    if (i >= 0) col[key] = i;
  }
  const missing = REQUIRED.filter((k) => col[k] == null);
  if (missing.length) {
    return {
      rows: [],
      errors: [{ line: 1, message: `Faltam as colunas: ${missing.map((k) => LABEL[k]).join(", ")}. Use o modelo como base.` }],
    };
  }
  const rows: CsvRow[] = [];
  const errors: CsvError[] = [];
  lines.slice(1).forEach((raw, i) => {
    const line = i + 2;
    if (!raw.trim() || split(raw, sep).every((c) => !c)) return;
    const f = split(raw, sep);
    const get = (k: string) => (col[k] == null ? "" : (f[col[k]] ?? "").trim());
    const fail = (message: string) => errors.push({ line, message });
    const date = parseDate(get("date"));
    if (!date) return fail(`data inválida "${get("date")}" (use dd/mm/aaaa)`);
    const account = get("account");
    if (!account) return fail("falta a conta");
    const asset = get("asset").toUpperCase();
    if (!asset) return fail("falta o ativo");
    const side = parseSide(get("side"));
    if (!side) return fail(`operação "${get("side")}" não reconhecida (use Compra ou Venda)`);
    const quantity = parseNumber(get("quantity"), sep);
    if (!quantity || Number(quantity) <= 0) return fail(`quantidade inválida "${get("quantity")}"`);
    const price = parseNumber(get("price"), sep);
    if (!price || Number(price) <= 0) return fail(`preço inválido "${get("price")}"`);
    const optional = (k: string): string | null | undefined => {
      const v = get(k);
      if (!v) return null;
      const n = parseNumber(v, sep);
      if (n == null || Number(n) < 0) {
        fail(`${k === "fees" ? "taxas" : k.toUpperCase()} inválido "${v}"`);
        return undefined;
      }
      return n;
    };
    const fees = optional("fees");
    const ir = optional("ir");
    const iof = optional("iof");
    if (fees === undefined || ir === undefined || iof === undefined) return;
    const cur = plain(get("currency"));
    const currency = !cur ? null : ["brl", "real", "reais", "r"].includes(cur) ? "BRL" : ["usd", "dolar", "dolares", "us"].includes(cur) ? "USD" : undefined;
    if (currency === undefined) return fail(`moeda "${get("currency")}" não reconhecida (use BRL ou USD)`);
    rows.push({ line, date, account, asset, side, quantity, price, fees, currency, ir, iof, note: get("note") || null });
  });
  return { rows, errors };
}

/** Modelo para baixar: cabeçalho e linhas de exemplo fictícias. */
export const CSV_TEMPLATE =
  "data;conta;ativo;operacao;quantidade;preco;taxas;moeda;ir;iof;observacao\n" +
  "02/03/2026;Corretora Exemplo;ABCD3;Compra;100;10,50;;BRL;;;taxas da B3 calculadas pelo app\n" +
  "15/04/2026;Corretora Exemplo;ABCD3;Venda;40;12,30;0,35;BRL;;;taxas informadas\n" +
  "10/03/2026;Exterior Exemplo;VOO;Compra;0,5;480,1234;0;USD;;;\n";
