// Tabelas das telas no formato que o app grava como planilha do Excel.
import { invoke } from "@tauri-apps/api/core";

export type XCell = string | null | { n: number; f: "money" | "pct" | "qty" } | { d: string };
export type XRow = { style: "head" | "group" | "row" | "total" | "grand" | "flow"; cells: XCell[] };
export type XSheet = {
  name: string;
  title: string;
  note: string;
  widths: number[];
  rows: XRow[];
  freeze_cols?: number;
  filter?: boolean;
};

export const money = (v: string | number | null | undefined): XCell => (v == null || v === "" ? null : { n: Number(v), f: "money" });
export const pct = (v: string | number | null | undefined): XCell => (v == null || v === "" ? null : { n: Number(v), f: "pct" });
export const qty = (v: string | number | null | undefined): XCell => (v == null || v === "" ? null : { n: Number(v), f: "qty" });
export const day = (iso: string): XCell => ({ d: iso });

/** "11/11/2026 15:45", para dizer na planilha quando ela foi feita. */
export function stamp(now = new Date()): string {
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(now.getDate())}/${p(now.getMonth() + 1)}/${now.getFullYear()} ${p(now.getHours())}:${p(now.getMinutes())}`;
}

/** Pergunta onde salvar e grava. `false` quando a pessoa cancela. */
export const saveXlsx = (name: string, sheets: XSheet[]) => invoke<boolean>("save_xlsx", { name, sheets });
