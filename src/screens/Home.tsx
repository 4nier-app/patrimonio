import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { api, errorMessage, market, type UpdateLine, type VaultStatus } from "../api";
import Logo from "../components/Logo";
import Audit from "./Audit";
import Dashboard from "./Dashboard";
import Operations from "./Operations";
import Portfolio from "./Portfolio";
import Returns from "./Returns";
import Security from "./Security";

// A leitura de PDF (pdf.js) só carrega quando a aba é aberta.
const ImportNotes = lazy(() => import("./ImportNotes"));

const TABS = [
  { id: "painel", label: "Painel" },
  { id: "carteira", label: "Carteira" },
  { id: "lancamentos", label: "Lançamentos" },
  { id: "importar", label: "Importar notas" },
  { id: "rentabilidade", label: "Rentabilidade" },
  { id: "auditoria", label: "Auditoria" },
  { id: "seguranca", label: "Segurança" },
] as const;

type Tab = (typeof TABS)[number]["id"];

type Props = { status: VaultStatus; onLocked: () => void; onChanged: () => void };

export default function Home({ status, onLocked, onChanged }: Props) {
  const [tab, setTab] = useState<Tab>("painel");
  const [updating, setUpdating] = useState(false);
  const [report, setReport] = useState<UpdateLine[] | null>(null);
  // Muda a cada atualização de cotações para as telas recarregarem.
  const [refresh, setRefresh] = useState(0);

  async function lock() {
    await api.lock();
    onLocked();
  }

  // Um pedido durante uma atualização roda de novo quando ela terminar.
  const running = useRef(false);
  const again = useRef(false);

  async function updateQuotes() {
    if (running.current) {
      again.current = true;
      return;
    }
    running.current = true;
    setUpdating(true);
    setReport(null);
    try {
      do {
        again.current = false;
        try {
          setReport(await market.updateQuotes());
        } catch (e) {
          setReport([{ source: "Cotações", ok: false, message: errorMessage(e) }]);
        }
      } while (again.current);
    } finally {
      running.current = false;
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

      {report && report.length > 0 && (
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
        {tab === "painel" && <Dashboard refresh={refresh} />}
        {tab === "carteira" && <Portfolio refresh={refresh} />}
        {tab === "lancamentos" && <Operations onChanged={() => void updateQuotes()} />}
        {tab === "importar" && (
          <Suspense fallback={<p className="muted">Carregando…</p>}>
            <ImportNotes onImported={() => void updateQuotes()} />
          </Suspense>
        )}
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
