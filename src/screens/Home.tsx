import { lazy, Suspense, useEffect, useRef, useState } from "react";
import { api, errorMessage, market, taxApi, type BackupReminder, type TaxView, type UpdateLine, type VaultStatus } from "../api";
import { date, money } from "../format";
import Logo from "../components/Logo";
import ErrorBoundary from "../components/ErrorBoundary";
import UndoToast from "../components/UndoToast";
import AssetPage from "./AssetPage";
import Audit from "./Audit";
import Incomes from "./Incomes";
import Broker, { rememberAsset } from "./Broker";
import Dashboard from "./Dashboard";
import Panel from "./Panel";
import Events from "./Events";
import Evolution from "./Evolution";
import FirstUse from "./FirstUse";
import Operations from "./Operations";
import Portfolio from "./Portfolio";
import Returns from "./Returns";
import Savings from "./Savings";
import Owned from "./Owned";
import ImportCsv from "./ImportCsv";
import ImportB3 from "./ImportB3";
import Allocation from "./Allocation";
import Security from "./Security";
import Taxes, { pendingDarfs } from "./Taxes";
import { isSavings } from "../currency";
import { setThemePref, themePref, type ThemePref } from "../theme";

// A leitura de PDF (pdf.js) só carrega quando a aba é aberta.
const ImportNotes = lazy(() => import("./ImportNotes"));

const TABS = [
  { id: "painel", label: "Painel" },
  { id: "evolucao", label: "Evolução" },
  { id: "carteira", label: "Carteira" },
  { id: "homebroker", label: "Home broker" },
  { id: "lancamentos", label: "Lançamentos" },
  { id: "impostos", label: "Impostos" },
  { id: "seguranca", label: "Segurança" },
] as const;

type Tab = (typeof TABS)[number]["id"];

// Abas com seções internas, para o menu principal ficar enxuto.
const SECTIONS = {
  carteira: [
    { id: "posicoes", label: "Posições" },
    { id: "rentabilidade", label: "Rentabilidade" },
    { id: "alocacao", label: "Alocação" },
  ],
  evolucao: [
    { id: "patrimonio", label: "Patrimônio" },
    { id: "saldos", label: "Contas e reservas" },
    { id: "bens", label: "Bens" },
  ],
  lancamentos: [
    { id: "manual", label: "Incluir e editar" },
    { id: "importar", label: "Importar notas" },
    { id: "b3", label: "Importar da B3" },
    { id: "planilha", label: "Importar planilha" },
    { id: "eventos", label: "Eventos (bonificação, desdobramento)" },
    { id: "proventos", label: "Proventos" },
  ],
  seguranca: [
    { id: "seguranca", label: "Senha e backup" },
    { id: "auditoria", label: "Auditoria" },
  ],
} as const;

type Section = (typeof SECTIONS)[keyof typeof SECTIONS][number]["id"];

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
    // Cofrinho não tem gráfico de mercado: os detalhes ficam em Saldos.
    if (isSavings(a)) return setTab("evolucao", "saldos");
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
  const [section, setSection] = useState<Record<string, Section>>({ carteira: "posicoes", evolucao: "patrimonio", lancamentos: "manual", seguranca: "seguranca" });
  function setTab(t: Tab, s?: Section) {
    if (assetRef.current && window.history.state?.asset) window.history.back();
    setAsset(null);
    setTabState(t);
    if (s) setSection((m) => ({ ...m, [t]: s }));
    // Entre chaves: no Windows, window.scrollTo devolve uma Promise.
    window.scrollTo({ top: 0 });
  }
  const sections = tab in SECTIONS ? SECTIONS[tab as keyof typeof SECTIONS] : null;
  const current = section[tab];
  const [theme, setTheme] = useState<ThemePref>(themePref);
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

  // Avisa quando há versão mais nova publicada (pode ser desligado em Segurança).
  const [newVersion, setNewVersion] = useState<string | null>(null);
  useEffect(() => {
    api
      .checkUpdate()
      .then((u) => setNewVersion(u?.version ?? null))
      .catch(() => {});
  }, []);

  // Perfil novo e vazio: o primeiro uso guiado ocupa a tela até terminar ou pular.
  const [firstUse, setFirstUse] = useState(false);
  useEffect(() => {
    api
      .firstUse()
      .then(setFirstUse)
      .catch(() => {});
  }, []);

  // Lembrete de backup fora deste computador.
  const [reminder, setReminder] = useState<BackupReminder | null>(null);
  useEffect(() => {
    api
      .backupReminder()
      .then(setReminder)
      .catch(() => {});
  }, [refresh, tab]);

  async function backupNow() {
    try {
      const info = await api.exportBackup();
      if (!info) return;
      setReminder(null);
      setReport([{ source: "Backup", ok: true, message: `gravado em ${info.path}. Guarde uma cópia fora deste computador.` }]);
    } catch (e) {
      setReport([{ source: "Backup", ok: false, message: errorMessage(e) }]);
    }
  }

  return (
    <div className="shell">
      <header className="topbar">
        <div className="topbar-row">
          <Logo />
          <div className="topbar-actions">
            <select
              className="theme-pick"
              aria-label="Tema"
              title="Tema do aplicativo"
              value={theme}
              onChange={(e) => {
                const t = e.target.value as ThemePref;
                setTheme(t);
                setThemePref(t);
              }}
            >
              <option value="auto">◐ Tema automático</option>
              <option value="light">☀ Tema claro</option>
              <option value="dark">☾ Tema escuro</option>
            </select>
            <button className="secondary" onClick={updateQuotes} disabled={updating} title="As cotações já são atualizadas ao abrir e depois de cada lançamento; use para buscar de novo agora">
              {updating ? "Atualizando…" : "↻ Atualizar cotações"}
            </button>
            <button className="secondary" onClick={lock} title="Fecha a carteira; na tela de entrada dá para trocar de perfil">
              {status.profiles.length > 1 ? `Bloquear (${status.profiles.find((p) => p.id === status.profile)?.name})` : "Bloquear"}
            </button>
          </div>
        </div>
        {!firstUse && (
        <nav>
          {TABS.map((t) => (
            <button key={t.id} className={tab === t.id ? "tab active" : "tab"} onClick={() => setTab(t.id)}>
              {t.label}
            </button>
          ))}
          <span className="app-version" title="Versão instalada do 4Nier">
            Versão {status.app_version}
          </span>
        </nav>
        )}
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

      {newVersion && (
        <div className="banner info" role="status">
          Nova versão {newVersion} do 4Nier disponível. Baixe e instale por cima; seus dados são mantidos.{" "}
          <button className="link" onClick={() => void api.openReleases()}>
            Baixar
          </button>{" "}
          <button className="link" onClick={() => setNewVersion(null)} aria-label="Fechar aviso">
            ✕
          </button>
        </div>
      )}

      {reminder && !firstUse && (
        <div className="banner warning" role="status">
          {reminder.last_backup && reminder.days != null
            ? `Seu último backup fora deste computador foi há ${reminder.days} dias (${date(reminder.last_backup.slice(0, 10))}).`
            : "Você ainda não salvou um backup fora deste computador."}{" "}
          Se o computador quebrar ou for roubado, os backups automáticos vão junto.{" "}
          <button className="link" onClick={() => void backupNow()}>
            Fazer backup agora
          </button>{" "}
          <button
            className="link subtle"
            onClick={() => {
              setReminder(null);
              api.snoozeBackupReminder().catch(() => {});
            }}
          >
            Lembrar daqui a 7 dias
          </button>
        </div>
      )}

      {!status.has_recovery_key && tab !== "seguranca" && !firstUse && (
        <div className="banner warning" role="status">
          Sua carteira ainda não tem chave de recuperação. Sem ela, se você esquecer a senha, não há como abrir os dados.{" "}
          <button className="link" onClick={() => setTab("seguranca", "seguranca")}>
            Criar agora
          </button>
        </div>
      )}

      {darfs.length > 0 && tab !== "impostos" && tab !== "painel" && (
        <div className={`banner warning${darfs.some((m) => m.status === "ATRASADO") ? " overdue" : ""}`} role="status">
          {darfs.length === 1
            ? `DARF de ${money(darfs[0].darf)} ${darfs[0].status === "ATRASADO" ? "venceu" : "vence"} em ${date(darfs[0].due_date)}.`
            : `${darfs.length} DARFs a pagar, somando ${money(String(darfs.reduce((t, m) => t + Number(m.darf), 0)))}.`} É uma estimativa: confira com um contador.{" "}
          <button className="link" onClick={() => setTab("impostos")}>
            Ver Impostos
          </button>
        </div>
      )}

      <main className="content">
        {firstUse && (
          <FirstUse
            onDone={() => {
              setFirstUse(false);
              setTab("painel");
              void updateQuotes();
            }}
          />
        )}
        {!firstUse && (
        <ErrorBoundary resetKey={`${tab}|${current ?? ""}|${asset ?? ""}`} onReset={() => (asset ? closeAsset() : setTab("painel"))}>
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
          {!asset && tab === "painel" && <Panel refresh={refresh} go={setTab} hasRecovery={status.has_recovery_key} darfs={darfs} />}
          {!asset && tab === "homebroker" && <Broker refresh={refresh} onOpen={openAsset} />}
          {!asset && sections && (
            <div className="subtabs" role="tablist">
              {sections.map((x) => (
                <button
                  key={x.id}
                  role="tab"
                  aria-selected={current === x.id}
                  className={current === x.id ? "active" : ""}
                  onClick={() => setSection((m) => ({ ...m, [tab]: x.id }))}
                >
                  {x.label}
                </button>
              ))}
            </div>
          )}
          {!asset && tab === "lancamentos" && current === "manual" && <Operations onChanged={() => void updateQuotes()} />}
          {!asset && tab === "lancamentos" && current === "proventos" && <Incomes refresh={refresh} onChanged={loadTaxes} />}
          {!asset && tab === "lancamentos" && current === "planilha" && <ImportCsv onImported={() => void updateQuotes()} />}
          {!asset && tab === "lancamentos" && current === "b3" && <ImportB3 onImported={() => void updateQuotes()} />}
          {!asset && tab === "lancamentos" && current === "importar" && (
            <Suspense fallback={<p className="muted">Carregando…</p>}>
              <ImportNotes onImported={() => void updateQuotes()} />
            </Suspense>
          )}
          {!asset && tab === "evolucao" && current === "patrimonio" && (
            <Evolution key={refresh} />
          )}
          {!asset && tab === "evolucao" && current === "saldos" && <Savings onChanged={() => void updateQuotes()} />}
          {!asset && tab === "evolucao" && current === "bens" && <Owned onChanged={() => setRefresh((n) => n + 1)} />}
          {!asset && tab === "lancamentos" && current === "eventos" && (
            <Events onChanged={() => void updateQuotes()} onIncomes={() => setTab("lancamentos", "proventos")} />
          )}
          {!asset && tab === "carteira" && current === "posicoes" && <Portfolio refresh={refresh} onOpen={openAsset} />}
          {!asset && tab === "carteira" && current === "rentabilidade" && (
            <>
              <Returns key={refresh} onToday={() => setTab("homebroker")} />
              <Dashboard refresh={refresh} />
            </>
          )}
          {!asset && tab === "carteira" && current === "alocacao" && <Allocation refresh={refresh} />}
          {!asset && tab === "impostos" && <Taxes refresh={refresh} onChanged={loadTaxes} />}
          {!asset && tab === "seguranca" && current === "auditoria" && <Audit status={status} />}
          {!asset && tab === "seguranca" && current === "seguranca" && (
            <Security
              status={status}
              onErased={onLocked}
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
        )}
      </main>
      <UndoToast />
    </div>
  );
}
