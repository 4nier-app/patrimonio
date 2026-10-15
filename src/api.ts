// Ponte tipada com os comandos do núcleo Rust (src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";

export type VaultStatus = {
  exists: boolean;
  /** O cofre tem chave de recuperação ("Esqueci minha senha"). */
  has_recovery_key: boolean;
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

export type RecoveryStatus = { configured: boolean; email: string | null; created_at: string | null };

export type BackupStatus = { auto_dir: string; last_auto: string | null; last_backup: string | null };

export type BackupInfo = { path: string; created_at: string; events: number; bytes: number };

export type BackupSummary = {
  path: string;
  created_at: string;
  app_version: string;
  schema_version: number;
  events: number;
  has_recovery_key: boolean;
};

export type RestoreResult = { previous: string | null };

export type IntegrityReport = { ok: boolean; events_checked: number; problem: string | null };

export type VersionEntry = { version: string; installed_at: string; changelog: string };

export const api = {
  vaultStatus: () => invoke<VaultStatus>("vault_status"),
  /** Cria o cofre e devolve a chave de recuperação gerada. */
  createVault: (password: string, confirmation: string, email: string) =>
    invoke<string>("create_vault", { password, confirmation, email }),
  resetPassword: (recoveryKey: string, next: string, confirmation: string) =>
    invoke<void>("reset_password", { recoveryKey, new: next, confirmation }),
  recoveryStatus: () => invoke<RecoveryStatus>("recovery_status"),
  generateRecoveryKey: (email: string) => invoke<string>("generate_recovery_key", { email }),
  sendRecoveryEmail: (email: string, key: string) => invoke<void>("send_recovery_email", { email, key }),
  backupStatus: () => invoke<BackupStatus>("backup_status"),
  /** Abre a janela "Salvar como"; null se o usuário cancelar. */
  exportBackup: () => invoke<BackupInfo | null>("export_backup"),
  /** Abre a janela "Abrir"; null se o usuário cancelar. */
  pickBackup: () => invoke<BackupSummary | null>("pick_backup"),
  restoreBackup: (path: string, password: string) => invoke<RestoreResult>("restore_backup", { path, password }),
  openDataFolder: () => invoke<void>("open_data_folder"),
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
export type Side = "COMPRA" | "VENDA" | "TRANSFERENCIA";
export type Currency = "BRL" | "USD";
export type Account = { name: string; currency: Currency };

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
  /** Moeda da conta. Para conta existente, o núcleo usa a moeda dela. */
  currency: Currency;
  /** Transferência de custódia: conta de origem. */
  from_account: string | null;
  /** Transferência de custódia: quantidade que ficou na origem e sai da carteira. */
  residue: Dec | null;
};

export type PricedOperation = { operation: Operation; gross: Dec; fees: Dec; fee_origin: string };

export type NoteFee = { kind: string; rule_id: string; base: Dec; rate: Dec; amount: Dec; formula: string };

export type Preview = { valid: string | null; gross: Dec; fees: Dec; fee_origin: string; note_fees: NoteFee[]; note_operations: number };

export type Quote = { id: number; asset: string; date: string; close: Dec; source: string };

export type FxRate = { date: string; rate: Dec; source: string };

export type Position = {
  asset: string;
  currency: Currency;
  quantity: Dec;
  cost: Dec;
  average_price: Dec;
  quote: Quote | null;
  market_value: Dec | null;
  unrealized: Dec | null;
  realized: Dec;
  oversold: boolean;
  transfer_residue: Dec;
  cost_brl: Dec | null;
  market_value_brl: Dec | null;
  unrealized_brl: Dec | null;
  realized_brl: Dec | null;
  fx: FxRate | null;
};

export type PortfolioView = {
  as_of: string;
  positions: Position[];
  /** Totais em reais (exterior convertido pela PTAX); vazios se faltar PTAX. */
  cost: Dec | null;
  market_value: Dec | null;
  unrealized: Dec | null;
  realized: Dec | null;
  fx: FxRate | null;
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
  currency: Currency;
  formula_id: string;
  formula_version: number;
  explanation: string[];
};

export type YearResult = { year: number; profit: Dec | null; rate: Dec | null; months: number; explanation: string };
export type NativeReturns = {
  months: MonthResult[];
  years: YearResult[];
  /** Efeito do câmbio, alinhado com os meses e anos em reais. */
  fx_months: (Dec | null)[];
  fx_years: (Dec | null)[];
};
export type ScopeReturns = {
  asset: string | null;
  currency: Currency;
  months: MonthResult[];
  years: YearResult[];
  native: NativeReturns | null;
};
export type ReturnsView = { as_of: string; portfolio: ScopeReturns; assets: ScopeReturns[] };

export type UpdateLine = { source: string; ok: boolean; message: string };
export type QuoteUpdate = { started_at: string; source: string; ok: boolean; items: number; message: string };

export const market = {
  searchAssets: (query: string) => invoke<Asset[]>("search_assets", { query }),
  listAccounts: () => invoke<Account[]>("list_accounts"),
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

// ---------- notas de corretagem (etapa 4) ----------

export type NoteStatus = { hash: string; imported: boolean; imported_at: string | null };
export type NoteImport = {
  hash: string;
  broker: string;
  number: string | null;
  date: string;
  account: string;
  operations: Operation[];
};

export const notesApi = {
  status: (hashes: string[]) => invoke<NoteStatus[]>("note_status", { hashes }),
  resolveNames: (items: [string, string][]) => invoke<string[][]>("resolve_b3_names", { items }),
  importNote: (note: NoteImport) => invoke<number>("import_note", { note }),
};

// ---------- painel (etapa 3) ----------

export type DashMonth = {
  year: number;
  month: number;
  period_start: string;
  period_end: string;
  value: Dec | null;
  invested: Dec;
  profit: Dec | null;
  rate: Dec | null;
  drawdown: Dec | null;
  explanation: string[];
};

export type Card = { label: string; from: string; to: string; rate: Dec | null; cdi: Dec | null; explanation: string[] };

export type Benchmark = {
  id: string;
  name: string;
  kind: "DAILY_RATE" | "MONTHLY_RATE" | "LEVEL";
  source: string;
  rates: (Dec | null)[];
  same_flows: (Dec | null)[];
  explanations: string[][];
};

export type Slice = { label: string; value: Dec | null; assets: { asset: string; value: Dec | null }[] };
export type AssetMonths = { asset: string; profits: (Dec | null)[] };
export type Span = { asset: string; accounts: string[]; start: string; end: string | null };

export type DashboardView = {
  as_of: string;
  months: DashMonth[];
  cards: Card[];
  benchmarks: Benchmark[];
  allocation: Slice[];
  contributions: AssetMonths[];
  positions: Position[];
  timeline: Span[];
};

export const dashboardApi = {
  load: () => invoke<DashboardView>("dashboard"),
};
