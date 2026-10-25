// Ponte tipada com os comandos do núcleo Rust (src-tauri/src/lib.rs).
import { invoke } from "@tauri-apps/api/core";

export type VaultStatus = {
  exists: boolean;
  /** O cofre tem chave de recuperação ("Esqueci minha senha"). */
  has_recovery_key: boolean;
  unlocked: boolean;
  app_version: string;
  data_dir: string;
  /** Perfil atual (cada pessoa da família tem o próprio cofre). */
  profile: string;
  profiles: Profile[];
};

export type Profile = { id: string; name: string; exists: boolean };

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

export type BackupReminder = { last_backup: string | null; days: number | null };
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
  selectProfile: (id: string) => invoke<void>("select_profile", { id }),
  createProfile: (name: string) => invoke<void>("create_profile", { name }),
  renameProfile: (id: string, name: string) => invoke<void>("rename_profile", { id, name }),
  removeProfile: (id: string) => invoke<void>("remove_profile", { id }),
  eraseProfile: (password: string) => invoke<void>("erase_profile", { password }),
  /** Cria o cofre e devolve a chave de recuperação gerada. */
  createVault: (password: string, confirmation: string, email: string) =>
    invoke<string>("create_vault", { password, confirmation, email }),
  resetPassword: (recoveryKey: string, next: string, confirmation: string) =>
    invoke<void>("reset_password", { recoveryKey, new: next, confirmation }),
  recoveryStatus: () => invoke<RecoveryStatus>("recovery_status"),
  generateRecoveryKey: (email: string) => invoke<string>("generate_recovery_key", { email }),
  sendRecoveryEmail: (email: string, key: string) => invoke<void>("send_recovery_email", { email, key }),
  backupStatus: () => invoke<BackupStatus>("backup_status"),
  /** Aviso de backup fora do computador; null quando não precisa avisar. */
  backupReminder: () => invoke<BackupReminder | null>("backup_reminder"),
  snoozeBackupReminder: () => invoke<void>("snooze_backup_reminder"),
  /** Perfil vazio que ainda não passou pelo primeiro uso guiado. */
  firstUse: () => invoke<boolean>("first_use"),
  finishFirstUse: () => invoke<void>("finish_first_use"),
  /** Abre a janela "Salvar como"; null se o usuário cancelar. */
  exportBackup: () => invoke<BackupInfo | null>("export_backup"),
  /** Abre a janela "Abrir"; null se o usuário cancelar. */
  pickBackup: () => invoke<BackupSummary | null>("pick_backup"),
  restoreBackup: (path: string, password: string) => invoke<RestoreResult>("restore_backup", { path, password }),
  openDataFolder: () => invoke<void>("open_data_folder"),
  /** Versão mais nova publicada no GitHub, ou null (sem novidade, sem internet ou aviso desligado). */
  checkUpdate: () => invoke<{ version: string } | null>("check_update"),
  updateNotice: () => invoke<boolean>("update_notice"),
  setUpdateNotice: (on: boolean) => invoke<void>("set_update_notice", { on }),
  openReleases: () => invoke<void>("open_releases"),
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
/** AJUSTE: efeito de um evento corporativo (bonificação, desdobramento, grupamento). */
export type Side = "COMPRA" | "VENDA" | "TRANSFERENCIA" | "AJUSTE";
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
  /** Fundo de investimento: IR retido no resgate ou no come-cotas. */
  ir?: Dec | null;
  /** Fundo de investimento: IOF retido no resgate. */
  iof?: Dec | null;
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
  /** PTAX da data de avaliação (a dos impostos). */
  fx: FxRate | null;
  /** Câmbio do valor em reais: PTAX ou valor de resgate (origem começa com "RESGATE"). */
  value_fx: FxRate | null;
  accounts: string[];
};

/** Saldo de uma corretora em dólar e quanto chegaria no Brasil. */
export type DollarAccount = {
  account: string;
  usd: Dec | null;
  fee_pct: Dec | null;
  iof_pct: Dec;
  total_pct: Dec | null;
  commercial: FxRate | null;
  commercial_updated: string | null;
  value_fx: FxRate | null;
  value_brl: Dec | null;
  ptax: FxRate | null;
  ptax_brl: Dec | null;
};

/** O câmbio é o valor de resgate (dólar comercial menos as taxas)? */
export function isResgate(fx: FxRate | null | undefined): boolean {
  return !!fx && fx.source.startsWith("RESGATE");
}

/** "dólar comercial 5,4350 (16/10/2026) − 1,38% (conversão 1,00% + IOF 0,38%)" */
export function resgateHow(fx: FxRate): string {
  return fx.source.replace(/^RESGATE:\s*/, "");
}

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
  /** Nome de cada ativo no catálogo (fundos aparecem pelo CNPJ). */
  names: Record<string, string>;
  /** Tipo de cada ativo: ACAO, FII, ETF, BDR, FUNDO, CRIPTO, OUTRO. */
  kinds: Record<string, string>;
  /** Corretoras em dólar (sem criptomoedas). */
  dollar_accounts: DollarAccount[];
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
export type ReturnsView = { as_of: string; portfolio: ScopeReturns; assets: ScopeReturns[]; names: Record<string, string> };

export type UpdateLine = { source: string; ok: boolean; message: string };
export type QuoteUpdate = { started_at: string; source: string; ok: boolean; items: number; message: string };

export const market = {
  searchAssets: (query: string) => invoke<Asset[]>("search_assets", { query }),
  searchFunds: (query: string) => invoke<Asset[]>("search_funds", { query }),
  nameFund: (ticker: string, name: string) => invoke<void>("name_fund", { ticker, name }),
  updateFundCatalog: () => invoke<UpdateLine>("update_fund_catalog"),
  updateTesouro: () => invoke<UpdateLine>("update_tesouro"),
  listTesouro: () => invoke<Asset[]>("list_tesouro"),
  listAccounts: () => invoke<Account[]>("list_accounts"),
  listOperations: () => invoke<PricedOperation[]>("list_operations"),
  preview: (operation: Operation) => invoke<Preview>("preview_operation", { operation }),
  create: (operation: Operation) => invoke<Operation>("create_operation", { operation }),
  update: (operation: Operation, reason: string) => invoke<Operation>("update_operation", { operation, reason }),
  remove: (id: string) => invoke<void>("remove_operation", { id, reason: null }),
  removeMany: (ids: string[]) => invoke<number>("remove_operations", { ids }),
  restore: (ids: string[]) => invoke<number>("restore_operations", { ids }),
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
  saveCsvTemplate: (content: string) => invoke<boolean>("save_csv_template", { content }),
};

/** Uma compra ou venda do extrato de negociação da B3. */
export type B3Trade = {
  /** Linha na planilha (a 1 é o cabeçalho). */
  line: number;
  date: string;
  side: "COMPRA" | "VENDA";
  market: string;
  institution: string;
  ticker: string;
  quantity: Dec;
  price: Dec;
  value: Dec;
};

export type B3Skipped = { line: number; text: string; reason: string };

export const b3Api = {
  readTrades: (bytes: Uint8Array) => invoke<{ trades: B3Trade[]; skipped: B3Skipped[] }>("read_b3_trades", { bytes: Array.from(bytes) }),
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
  /** Nome de cada ativo no catálogo (fundos aparecem pelo CNPJ). */
  names: Record<string, string>;
  /** FGTS, conta corrente e cartões benefício: patrimônio fora da rentabilidade. */
  other_balances: OtherBalance[];
};

export type OtherBalance = { id: string; name: string; account: string; kind: string; balance: Dec; last_informed: string | null };

/** Dias úteis de um mês: resultado de cada dia, no mesmo formato dos meses. */
export type DaysView = { days: DashMonth[]; benchmarks: Benchmark[]; contributions: AssetMonths[] };

export const dashboardApi = {
  load: () => invoke<DashboardView>("dashboard"),
  days: (year: number, month: number) => invoke<DaysView>("dashboard_days", { year, month }),
};

// ---------- evolução do patrimônio ----------

export type EvolutionColumn = { date: string; kind: "BASE" | "MONTH" | "YEAR" | "YESTERDAY" | "TODAY" };
export type EvolutionRow = {
  /** Código no motor; vazio para FGTS, conta e benefício (não são investimento). */
  asset: string | null;
  /** Saldo de Contas e reservas: dá para informar o valor clicando na célula. */
  savings_id: string | null;
  name: string;
  detail: string | null;
  values: (Dec | null)[];
  missing: boolean[];
  /** Coluna fora do período do saldo (antes do "desde" ou depois do "até"). */
  outside: boolean[];
  gain: EvolutionGain | null;
  /** Desde a primeira compra até a última coluna (só investimentos). */
  since: EvolutionGain | null;
  /** Conta do valor de resgate em cada coluna (corretora em dólar com custo informado). */
  notes: (string | null)[];
  /** "Nomad · valor de resgate" */
  value_label: string | null;
  /** Compras em aberto, mês a mês (vazio quando há uma compra só). */
  lots: EvolutionLot[];
};
/** Uma compra do ativo na Evolução; nas vendas, as compras mais antigas saem primeiro. */
export type EvolutionLot = {
  date: string;
  account: string;
  quantity: Dec;
  /** "ações", "cotas", "títulos" ou o código da cripto. */
  unit: string;
  values: (Dec | null)[];
  /** Custo em reais do que restava da compra: a base da % no mês da compra. */
  costs: (Dec | null)[];
  gain: EvolutionGain | null;
  since: EvolutionGain | null;
};
/** Resultado no período: valor final + vendas − compras − valor inicial; base = inicial + compras. */
export type EvolutionGain = { value: Dec; base: Dec };
export type EvolutionGroup = {
  id: string;
  label: string;
  investment: boolean;
  rows: EvolutionRow[];
  total: (Dec | null)[];
  gain: EvolutionGain | null;
  since: EvolutionGain | null;
};
export type EvolutionView = {
  years: number[];
  year: number | null;
  columns: EvolutionColumn[];
  groups: EvolutionGroup[];
  total: (Dec | null)[];
  investments: (Dec | null)[];
  /** Aportes − resgates dos investimentos desde a coluna anterior. */
  new_money: (Dec | null)[];
  total_gain: EvolutionGain | null;
  investments_gain: EvolutionGain | null;
  investments_since: EvolutionGain | null;
  incomplete: boolean;
};

export const evolutionApi = {
  load: (year: number | null) => invoke<EvolutionView>("evolution", { year }),
};

/** Alocação ideal: meta em % de cada classe (fundos, b3, fii, exterior, cripto, rf, fgts, dinheiro). */
export const allocationApi = {
  targets: () => invoke<Record<string, Dec>>("allocation_targets"),
  save: (targets: Record<string, Dec>) => invoke<void>("save_allocation_targets", { targets }),
};

// ---------- impostos ----------

export type TaxGroup = "COMUM" | "DAY_TRADE" | "FII";

export type SaleLine = {
  date: string;
  account: string;
  asset: string;
  group: TaxGroup;
  stock: boolean;
  day_trade: boolean;
  quantity: Dec;
  gross: Dec;
  net: Dec;
  cost: Dec;
  average_price: Dec;
  result: Dec;
};

export type GroupMonth = {
  group: TaxGroup;
  result: Dec;
  loss_before: Dec;
  loss_used: Dec;
  loss_after: Dec;
  base: Dec;
  rate: Dec;
  tax: Dec;
};

export type DarfPayment = { month: string; paid_on: string; amount: Dec };

export type DarfStatus = "PAGO" | "PENDENTE" | "ATRASADO" | "EM_ANDAMENTO" | "SEM_DARF";

export type TaxMonth = {
  month: string;
  stock_sales: Dec;
  exempt: boolean;
  exempt_profit: Dec;
  groups: GroupMonth[];
  gross_tax: Dec;
  irrf_month: Dec;
  irrf_before: Dec;
  irrf_used: Dec;
  irrf_after: Dec;
  small_before: Dec;
  due: Dec;
  darf: Dec;
  small_after: Dec;
  due_date: string;
  sales: SaleLine[];
  status: DarfStatus;
  paid: DarfPayment | null;
};

export type ForeignSale = {
  date: string;
  account: string;
  asset: string;
  crypto: boolean;
  quantity: Dec;
  net_usd: Dec;
  ptax: Dec;
  net_brl: Dec;
  cost_brl: Dec;
  result_brl: Dec;
};

export type ForeignYear = {
  year: number;
  result: Dec;
  loss_before: Dec;
  loss_used: Dec;
  loss_after: Dec;
  base: Dec;
  rate: Dec;
  tax: Dec;
  sales: ForeignSale[];
};

export type YearEndPosition = { asset: string; quantity: Dec; cost_brl: Dec; cost_usd: Dec | null };

export type TaxView = {
  months: TaxMonth[];
  annual: { foreign: ForeignYear[]; positions: Record<string, YearEndPosition[]>; warnings: string[] };
  names: Record<string, string>;
  warnings: string[];
  darf_code: string;
  savings_year_end: { year: number; name: string; account: string; kind: string; value: Dec }[];
};

export const taxApi = {
  load: () => invoke<TaxView>("taxes"),
  markPaid: (payment: DarfPayment) => invoke<void>("mark_darf_paid", { payment }),
  unmarkPaid: (month: string) => invoke<void>("unmark_darf_paid", { month }),
};

// ---------- página do ativo ----------

export type Venue = "B3" | "US" | "CRIPTO" | "FUNDO" | "TESOURO";
export type AverageStep = { date: string; quantity: Dec; average_price: Dec };
export type DailyClose = { date: string; close: Dec; source: string };
export type AssetDetail = {
  ticker: string;
  name: string;
  kind: string;
  currency: Currency;
  venue: Venue;
  position: Position | null;
  operations: PricedOperation[];
  average_steps: AverageStep[];
  history: DailyClose[];
  /** Compras em aberto pelo FIFO, com o ganho de cada uma (informativo; o IR usa o preço médio). */
  lots: LotView[];
};
export type LotView = {
  id: string;
  date: string;
  account: string;
  bought: Dec;
  quantity: Dec;
  unit_cost: Dec;
  cost: Dec;
  value: Dec | null;
  gain: Dec | null;
  cost_brl: Dec | null;
  value_brl: Dec | null;
  gain_brl: Dec | null;
};
export type Candle = { time: number; open: number; high: number; low: number; close: number; volume: number };
export type ChartPeriod = "1d" | "5d" | "1mo" | "6mo" | "1y" | "5y" | "max";
export type ChartData = {
  candles: Candle[];
  daily: boolean;
  price: number | null;
  price_time: number | null;
  previous_close: number | null;
  gmtoffset: number;
  source: string;
  offline: string | null;
};

export type DayMove = {
  asset: string;
  name: string;
  currency: Currency;
  venue: Venue;
  quantity: Dec;
  price: number | null;
  previous_close: number | null;
  change: number | null;
  result: number | null;
  result_brl: number | null;
  value_brl: number | null;
  price_time: number | null;
  quote_date: string | null;
  offline: string | null;
};
export type DayView = { items: DayMove[]; total_brl: number | null; total_change: number | null; fx: number | null };

export const assetApi = {
  dayMoves: () => invoke<DayView>("day_moves"),
  detail: (ticker: string) => invoke<AssetDetail>("asset_detail", { ticker }),
  chart: (ticker: string, period: ChartPeriod, interval: string) => invoke<ChartData>("asset_chart", { ticker, period, interval }),
};

// ---------- proventos (etapa 9) ----------

export type IncomeKind = "DIVIDENDO" | "JCP" | "RENDIMENTO" | "JUROS" | "OUTRO";

export type Income = {
  id: string;
  account: string;
  asset: string;
  kind: IncomeKind;
  pay_date: string;
  ex_date: string | null;
  gross: Dec;
  withheld: Dec;
  currency: "BRL" | "USD";
  note: string | null;
};

export type IncomeRow = {
  income: Income;
  net: Dec;
  gross_brl: Dec | null;
  withheld_brl: Dec | null;
  net_brl: Dec | null;
  fx: FxRate | null;
  quantity: Dec | null;
  per_share: Dec | null;
};

export type IncomesView = { items: IncomeRow[]; names: Record<string, string>; kinds: Record<string, string> };

export type EventKind = "BONIFICACAO" | "DESDOBRAMENTO" | "GRUPAMENTO";

export type CorporateEvent = {
  id: string;
  asset: string;
  kind: EventKind;
  data_com: string;
  /** Bonificação: %; desdobramento: cada ação vira N; grupamento: cada N viram 1. */
  proportion: Dec;
  cost_per_share: Dec;
  note: string | null;
};

export type EventRow = {
  event: CorporateEvent;
  label: string;
  ex_date: string;
  effects: { account: string; before: Dec; after: Dec; delta: Dec; cost_added: Dec }[];
  before: Dec;
  after: Dec;
  exempt_brl: Dec | null;
  average_before: Dec | null;
  average_after: Dec | null;
  currency: "BRL" | "USD";
};

export type EventsView = { items: EventRow[]; assets: string[]; names: Record<string, string> };

export const eventsApi = {
  load: () => invoke<EventsView>("corporate_events"),
  preview: (event: CorporateEvent) => invoke<EventRow>("preview_event", { event }),
  create: (event: CorporateEvent) => invoke<CorporateEvent>("create_event", { event }),
  update: (event: CorporateEvent) => invoke<CorporateEvent>("update_event", { event, reason: null }),
  remove: (id: string) => invoke<void>("remove_event", { id }),
  restore: (id: string) => invoke<CorporateEvent>("restore_event", { id }),
};

export const incomesApi = {
  load: () => invoke<IncomesView>("incomes"),
  create: (income: Income) => invoke<Income>("create_income", { income }),
  update: (income: Income, reason: string) => invoke<Income>("update_income", { income, reason }),
  remove: (id: string) => invoke<void>("remove_income", { id, reason: null }),
  restore: (id: string) => invoke<Income>("restore_income", { id }),
};

// ---------- saldos e reservas (etapa 10) ----------

export type SavingsKind = "COFRINHO" | "FGTS" | "CONTA" | "BENEFICIO" | "RENDA_FIXA";

/** Renda fixa (CDB, LCI, LCA, Tesouro): produto, taxa como texto e vencimento. */
export type FixedIncome = { product: string; rate: string | null; maturity: string | null };
export type SavingsMoveKind = "DEPOSITO" | "RESGATE" | "SALDO";

export type Savings = {
  id: string;
  kind: SavingsKind;
  account: string;
  name: string;
  rate_pct: Dec;
  tax_free: boolean;
  note: string | null;
  /** Mês de início ("AAAA-MM-01"). */
  since: string | null;
  /** Mês de encerramento ("AAAA-MM-01"), se a conta foi fechada. */
  until: string | null;
  /** Só na renda fixa. */
  fixed?: FixedIncome | null;
};

export type SavingsMove = {
  id: string;
  savings_id: string;
  date: string;
  kind: SavingsMoveKind;
  amount: Dec;
  ir: Dec | null;
  iof: Dec | null;
  note: string | null;
};

export type BalanceCheck = { move_id: string; date: string; informed: Dec; expected: Dec; difference: Dec; suspicious: boolean };
export type Redemption = { move_id: string; date: string; net: Dec; gross: Dec; ir: Dec; iof: Dec; estimated: boolean; gain: Dec };

export type SavingsItem = {
  savings: Savings;
  kind_label: string;
  ticker: string;
  balance: Dec;
  balance_net: Dec;
  invested: Dec;
  earned: Dec;
  last_informed: string | null;
  checks: BalanceCheck[];
  redemptions: Redemption[];
  moves: SavingsMove[];
};

export type SavingsView = { items: SavingsItem[]; total_investments: Dec; total_others: Dec; cdi_until: string | null };

export const savingsApi = {
  load: () => invoke<SavingsView>("savings"),
  create: (item: Savings) => invoke<Savings>("create_savings", { item }),
  update: (item: Savings) => invoke<Savings>("update_savings", { item }),
  remove: (id: string) => invoke<void>("remove_savings", { id }),
  restore: (id: string) => invoke<Savings>("restore_savings", { id }),
  createMove: (item: SavingsMove) => invoke<SavingsMove>("create_savings_move", { item }),
  updateMove: (item: SavingsMove) => invoke<SavingsMove>("update_savings_move", { item }),
  removeMove: (id: string) => invoke<void>("remove_savings_move", { id }),
  restoreMove: (id: string) => invoke<SavingsMove>("restore_savings_move", { id }),
};

// ---------- bens (imóvel e veículo) e corretagem (0.14.0) ----------

export type OwnedKind = "IMOVEL" | "VEICULO";

export type OwnedAsset = {
  id: string;
  kind: OwnedKind;
  name: string;
  bought_on: string;
  price: Dec;
  sold_on: string | null;
  note: string | null;
};

/** Obra ou reforma do imóvel: soma ao custo. */
export type Improvement = { id: string; owned_id: string; date: string; amount: Dec; description: string };

export type OwnedRow = { item: OwnedAsset; works: Improvement[]; cost: Dec };

export const ownedApi = {
  list: () => invoke<OwnedRow[]>("list_owned"),
  create: (item: OwnedAsset) => invoke<OwnedAsset>("create_owned", { item }),
  update: (item: OwnedAsset) => invoke<OwnedAsset>("update_owned", { item }),
  remove: (id: string) => invoke<void>("remove_owned", { id }),
  createWork: (item: Improvement) => invoke<Improvement>("create_improvement", { item }),
  updateWork: (item: Improvement) => invoke<Improvement>("update_improvement", { item }),
  removeWork: (id: string) => invoke<void>("remove_improvement", { id }),
};

/** Corretagem de uma conta nos lançamentos à mão, a partir de uma data. */
export type Brokerage = { per_order: Dec; iss_pct: Dec; since: string };

export const brokerageApi = {
  table: () => invoke<Record<string, Brokerage>>("brokerage_table"),
  set: (account: string, brokerage: Brokerage) => invoke<void>("set_brokerage", { account, brokerage }),
};

/** Custo para trazer ao Brasil (corretoras em dólar): taxa de conversão em %. O IOF fica no aplicativo. */
export const exitCostApi = {
  table: () => invoke<Record<string, Dec>>("exit_cost_table"),
  set: (account: string, feePct: Dec | null) => invoke<void>("set_exit_cost", { account, feePct }),
};

/** IOF na volta do dinheiro ao Brasil, em %. Igual ao do aplicativo (crates/storage/src/resgate.rs). */
export const IOF_PCT = "0.38";
