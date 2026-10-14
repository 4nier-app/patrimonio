// Ponte tipada com os comandos do núcleo Rust (src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";

export type VaultStatus = {
  exists: boolean;
  unlocked: boolean;
  app_version: string;
  data_dir: string;
};

export type Action = "CRIADA" | "ALTERADA" | "REMOVIDA";

export type LedgerEvent = {
  seq: number;
  occurred_at: string;
  entity: string;
  entity_id: string;
  action: Action;
  before: string | null;
  after: string | null;
  reason: string | null;
  app_version: string;
  prev_hash: string;
  hash: string;
};

export type IntegrityReport = { ok: boolean; events_checked: number; problem: string | null };

export type VersionEntry = { version: string; installed_at: string; changelog: string };

export const api = {
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  createVault: (password: string, confirmation: string) => invoke<void>("create_vault", { password, confirmation }),
  unlock: (password: string) => invoke<void>("unlock", { password }),
  lock: () => invoke<void>("lock"),
  changePassword: (current: string, next: string, confirmation: string) =>
    invoke<void>("change_password", { current, new: next, confirmation }),
  listEvents: (limit = 200, offset = 0) => invoke<LedgerEvent[]>("list_events", { limit, offset }),
  verifyIntegrity: () => invoke<IntegrityReport>("verify_integrity"),
  versionHistory: () => invoke<VersionEntry[]>("version_history"),
};

export function errorMessage(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : "erro inesperado";
}

// ---------- etapa 1 ----------

/** Decimais chegam e vão como texto exato ("1234.56"). */
export type Dec = string;
export type Side = "COMPRA" | "VENDA";

export type Asset = { ticker: string; name: string; kind: string; market: string; currency: string; isin: string | null; source: string };

export type Operation = {
  id: string;
  account: string;
  asset: string;
  date: string;
  side: Side;
  quantity: Dec;
  price: Dec;
  manual_fees: Dec | null;
  note: string | null;
};

export type PricedOperation = { operation: Operation; gross: Dec; fees: Dec; fee_origin: string };

export type NoteFee = { kind: string; rule_id: string; base: Dec; rate: Dec; amount: Dec; formula: string };

export type Preview = { valid: string | null; gross: Dec; fees: Dec; fee_origin: string; note_fees: NoteFee[]; note_operations: number };

export type Quote = { id: number; asset: string; date: string; close: Dec; source: string };

export type Position = {
  asset: string;
  quantity: Dec;
  cost: Dec;
  average_price: Dec;
  quote: Quote | null;
  market_value: Dec | null;
  unrealized: Dec | null;
  realized: Dec;
  oversold: boolean;
};

export type PortfolioView = {
  as_of: string;
  positions: Position[];
  cost: Dec;
  market_value: Dec | null;
  unrealized: Dec | null;
  realized: Dec;
  last_update: string | null;
};

export type MonthResult = {
  year: number;
  month: number;
  period_start: string;
  period_end: string;
  days: number;
  closed: boolean;
  net_flows: Dec;
  weighted_flows: Dec;
  profit: Dec | null;
  average_capital: Dec | null;
  rate: Dec | null;
  formula_id: string;
  formula_version: number;
  explanation: string[];
};

export type YearResult = { year: number; profit: Dec | null; rate: Dec | null; months: number; explanation: string };
export type ScopeReturns = { asset: string | null; months: MonthResult[]; years: YearResult[] };
export type ReturnsView = { as_of: string; portfolio: ScopeReturns; assets: ScopeReturns[] };

export type UpdateLine = { source: string; ok: boolean; message: string };
export type QuoteUpdate = { started_at: string; source: string; ok: boolean; items: number; message: string };

export const market = {
  searchAssets: (query: string) => invoke<Asset[]>("search_assets", { query }),
  listAccounts: () => invoke<string[]>("list_accounts"),
  listOperations: () => invoke<PricedOperation[]>("list_operations"),
  preview: (operation: Operation) => invoke<Preview>("preview_operation", { operation }),
  create: (operation: Operation) => invoke<Operation>("create_operation", { operation }),
  update: (operation: Operation, reason: string) => invoke<Operation>("update_operation", { operation, reason }),
  remove: (id: string, reason: string) => invoke<void>("remove_operation", { id, reason }),
  portfolio: () => invoke<PortfolioView>("portfolio"),
  returns: () => invoke<ReturnsView>("returns_report"),
  updateQuotes: () => invoke<UpdateLine[]>("update_quotes"),
  quoteUpdates: () => invoke<QuoteUpdate[]>("list_quote_updates"),
};
