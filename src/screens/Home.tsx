import { useEffect, useState } from "react";
import { api, errorMessage, market, type UpdateLine, type VaultStatus } from "../api";
import Logo from "../components/Logo";
import Audit from "./Audit";
import Operations from "./Operations";
import Portfolio from "./Portfolio";
import Returns from "./Returns";
import Security from "./Security";

const TABS = [
  { id: "carteira", label: "Carteira" },
  { id: "lancamentos", label: "Lançamentos" },
  { id: "rentabilidade", label: "Rentabilidade" },
  { id: "auditoria", label: "Auditoria" },
  { id: "seguranca", label: "Segurança" },
] as const;

type Tab = (typeof TABS)[number]["id"];

type Props = { status: VaultStatus; onLocked: () => void; onChanged: () => void };

export default function Home({ status, onLocked, onChanged }: Props) {
  const [tab, setTab] = useState<Tab>("carteira");
  const [updating, setUpdating] = useState(false);
  const [report, setReport] = useState<UpdateLine[] | null>(null);
  // Muda a cada atualização de cotações para as telas recarregarem.
  const [refresh, setRefresh] = useState(0);

  async function lock() {
    await api.lock();
    onLocked();
  }

  async function updateQuotes() {
    setUpdating(true);
    setReport(null);
    try {
      setReport(await market.updateQuotes());
    } catch (e) {
      setReport([{ source: "Cotações", ok: false, message: errorMessage(e) }]);
    } finally {
      setUpdating(false);
      setRefresh((n) => n + 1);
    }
  }

  // Ao abrir o cofre, atualiza catálogos (B3 e EUA), cotações e PTAX em
  // segundo plano; o que já está em dia não é baixado de novo.
  useEffect(() => {
    void updateQuotes();
  }, []);

  return (
    <div className="shell">
      <header className="topbar">
        <div className="topbar-row">
          <Logo />
          <div className="topbar-actions">
            <button onClick={updateQuotes} disabled={updating} title="Busca os preços mais recentes na B3 e no Yahoo">
              {updating ? "Atualizando…" : "↻ Atualizar cotações"}
            </button>
            <button className="secondary" onClick={lock}>
              Bloquear
            </button>
          </div>
        </div>
        <nav>
          {TABS.map((t) => (
            <button key={t.id} className={tab === t.id ? "tab active" : "tab"} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
        </nav>
      </header>

      {report && (
        <div className="update-report" role="status">
          <ul>
            {report.map((l) => (
              <li key={l.source} className={l.ok ? "ok" : "error"}>
                <span className="dot" /> <strong>{l.source}:</strong> {l.message}
              </li>
            ))}
          </ul>
          <button className="link" onClick={() => setReport(null)} aria-label="Fechar">
            ✕
          </button>
        </div>
      )}

      {!status.has_recovery_key && tab !== "seguranca" && (
        <div className="banner warning" role="status">
          Sua carteira ainda não tem chave de recuperação. Sem ela, se você esquecer a senha, não há como abrir os dados.{" "}
          <button className="link" onClick={() => setTab("seguranca")}>
            Criar agora
          </button>
        </div>
      )}

      <main className="content">
        {tab === "carteira" && <Portfolio refresh={refresh} />}
        {tab === "lancamentos" && <Operations />}
        {tab === "rentabilidade" && <Returns key={refresh} />}
        {tab === "auditoria" && <Audit status={status} />}
        {tab === "seguranca" && (
          <Security
            status={status}
            onChanged={onChanged}
            onRestored={() => {
              setTab("carteira");
              setReport([{ source: "Backup", ok: true, message: "restaurado e conferido; os dados do backup já estão em uso" }]);
              setRefresh((n) => n + 1);
              onChanged();
            }}
          />
        )}
      </main>
    </div>
  );
}
