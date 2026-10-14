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
