import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { api, errorMessage, market, taxApi, type TaxView, type UpdateLine, type VaultStatus } from "../api";
import { date, money } from "../format";
import Logo from "../components/Logo";
import ErrorBoundary from "../components/ErrorBoundary";
import AssetPage from "./AssetPage";
import Audit from "./Audit";
import Broker, { rememberAsset } from "./Broker";
import Dashboard from "./Dashboard";
import Operations from "./Operations";
import Portfolio from "./Portfolio";
import Returns from "./Returns";
import Security from "./Security";
import Taxes, { pendingDarfs } from "./Taxes";

// A leitura de PDF (pdf.js) só carrega quando a aba é aberta.
const ImportNotes = lazy(() => import("./ImportNotes"));

const TABS = [
  { id: "painel", label: "Painel" },
  { id: "carteira", label: "Carteira" },
  { id: "homebroker", label: "Home broker" },
  { id: "lancamentos", label: "Lançamentos" },
  { id: "importar", label: "Importar notas" },
  { id: "rentabilidade", label: "Rentabilidade" },
  { id: "impostos", label: "Impostos" },
  { id: "auditoria", label: "Auditoria" },
  { id: "seguranca", label: "Segurança" },
] as const;

type Tab = (typeof TABS)[number]["id"];

type Props = { status: VaultStatus; onLocked: () => void; onChanged: () => void };

export default function Home({ status, onLocked, onChanged }: Props) {
  const [tab, setTabState] = useState<Tab>("painel");
  // Página de um ativo aberta a partir da Carteira ou do Home broker.
  const [asset, setAssetState] = useState<string | null>(null);
  const assetRef = useRef<string | null>(null);
  function setAsset(a: string | null) {
    assetRef.current = a;
    setAssetState(a);
  }
  // Abrir um ativo entra no histórico da janela: o botão "voltar" do mouse ou
  // do teclado fecha a página do ativo em vez de sair do aplicativo.
  function openAsset(a: string) {
    if (!assetRef.current) window.history.pushState({ asset: a }, "");
    else window.history.replaceState({ asset: a }, "");
    setAsset(a);
  }
  function closeAsset() {
    if (!assetRef.current) return;
    if (window.history.state?.asset) window.history.back();
    else setAsset(null);
  }
  useEffect(() => {
    const onPop = () => setAsset(window.history.state?.asset ?? null);
    window.addEventListener("popstate", onPop);
    return () => window.removeEventListener("popstate", onPop);
  }, []);
  function setTab(t: Tab) {
    if (assetRef.current && window.history.state?.asset) window.history.back();
    setAsset(null);
    setTabState(t);
  }
  const [updating, setUpdating] = useState(false);
  const [report, setReport] = useState<UpdateLine[] | null>(null);
  // Muda a cada atualização de cotações para as telas recarregarem.
  const [refresh, setRefresh] = useState(0);
  const [taxes, setTaxes] = useState<TaxView | null>(null);

  function loadTaxes() {
    taxApi
      .load()
      .then(setTaxes)
      .catch(() => setTaxes(null));
  }

  useEffect(loadTaxes, [refresh]);
  const darfs = pendingDarfs(taxes);

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

      {darfs.length > 0 && tab !== "impostos" && (
        <div className={`banner warning${darfs.some((m) => m.status === "ATRASADO") ? " overdue" : ""}`} role="status">
          {darfs.length === 1
            ? `DARF de ${money(darfs[0].darf)} ${darfs[0].status === "ATRASADO" ? "venceu" : "vence"} em ${date(darfs[0].due_date)}.`
            : `${darfs.length} DARFs a pagar, somando ${money(String(darfs.reduce((t, m) => t + Number(m.darf), 0)))}.`}{" "}
          <button className="link" onClick={() => setTab("impostos")}>
            Ver Impostos
          </button>
        </div>
      )}

      <main className="content">
        <ErrorBoundary key={`${tab}|${asset ?? ""}`} onReset={() => (asset ? closeAsset() : setTab("painel"))}>
          {asset && (
            <AssetPage
              key={asset}
              ticker={asset}
              refresh={refresh}
              backLabel={tab === "homebroker" ? "Voltar ao Home broker" : tab === "carteira" ? "Voltar para a Carteira" : "Voltar"}
              onBack={closeAsset}
              onLoaded={(d) => rememberAsset(d.ticker, d.name)}
            />
          )}
          {!asset && tab === "painel" && <Dashboard refresh={refresh} />}
          {!asset && tab === "carteira" && <Portfolio refresh={refresh} onOpen={openAsset} />}
          {!asset && tab === "homebroker" && <Broker refresh={refresh} onOpen={openAsset} />}
          {!asset && tab === "lancamentos" && <Operations onChanged={() => void updateQuotes()} />}
          {!asset && tab === "importar" && (
            <Suspense fallback={<p className="muted">Carregando…</p>}>
              <ImportNotes onImported={() => void updateQuotes()} />
            </Suspense>
          )}
          {!asset && tab === "rentabilidade" && <Returns key={refresh} />}
          {!asset && tab === "impostos" && <Taxes refresh={refresh} onChanged={loadTaxes} />}
          {!asset && tab === "auditoria" && <Audit status={status} />}
          {!asset && tab === "seguranca" && (
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
        </ErrorBoundary>
      </main>
    </div>
  );
}
