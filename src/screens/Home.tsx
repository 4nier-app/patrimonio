import { useState } from "react";
import { api, type VaultStatus } from "../api";
import Audit from "./Audit";
import SystemHistory from "./SystemHistory";
import Security from "./Security";

const TABS = [
  { id: "auditoria", label: "Auditoria" },
  { id: "sistema", label: "Histórico do sistema" },
  { id: "seguranca", label: "Segurança" },
] as const;

type Tab = (typeof TABS)[number]["id"];

export default function Home({ status, onLocked }: { status: VaultStatus; onLocked: () => void }) {
  const [tab, setTab] = useState<Tab>("auditoria");

  async function lock() {
    await api.lock();
    onLocked();
  }

  return (
    <div className="shell">
      <header className="topbar">
        <strong>Carteira Auditável</strong>
        <nav>
          {TABS.map((t) => (
            <button key={t.id} className={tab === t.id ? "tab active" : "tab"} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
        </nav>
        <button className="secondary" onClick={lock}>Bloquear</button>
      </header>
      <main className="content">
        {tab === "auditoria" && <Audit />}
        {tab === "sistema" && <SystemHistory status={status} />}
        {tab === "seguranca" && <Security status={status} />}
      </main>
    </div>
  );
}
