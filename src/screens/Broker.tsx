import { useEffect, useState } from "react";
import { errorMessage, market, type Asset, type PortfolioView } from "../api";
import { isFund } from "../currency";
import { AssetName } from "./Portfolio";

const RECENT_KEY = "4nier.homebroker.recentes";
const RECENT_MAX = 12;

type Recent = { ticker: string; name: string };

function loadRecent(): Recent[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((x) => typeof x?.ticker === "string").slice(0, RECENT_MAX) : [];
  } catch {
    return [];
  }
}

/** Guarda o ativo aberto no topo da lista de vistos recentemente. */
export function rememberAsset(ticker: string, name: string) {
  try {
    const list = [{ ticker, name }, ...loadRecent().filter((x) => x.ticker !== ticker)].slice(0, RECENT_MAX);
    localStorage.setItem(RECENT_KEY, JSON.stringify(list));
  } catch {
    // Sem armazenamento: a lista de recentes só não aparece.
  }
}

const KIND: Record<string, string> = {
  ACAO: "Ação",
  FII: "FII",
  ETF: "ETF",
  BDR: "BDR",
  FUNDO: "Fundo",
  CRIPTO: "Cripto",
  STOCK: "Ação EUA",
};

function kindLabel(a: Asset): string {
  if (a.kind === "ACAO" && a.currency === "USD") return "Ação EUA";
  return KIND[a.kind] ?? a.kind;
}

/** Home broker: abre o gráfico de qualquer ativo do catálogo, tendo ou não na carteira. */
export default function Broker({ refresh, onOpen }: { refresh: number; onOpen: (asset: string) => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Asset[]>([]);
  const [searching, setSearching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<PortfolioView | null>(null);
  const [recent, setRecent] = useState<Recent[]>(loadRecent);

  useEffect(() => {
    market
      .portfolio()
      .then(setView)
      .catch(() => setView(null));
  }, [refresh]);

  useEffect(() => {
    const q = query.trim();
    if (q.length < 2) {
      setResults([]);
      return;
    }
    let alive = true;
    setSearching(true);
    const id = window.setTimeout(() => {
      market
        .searchAssets(q)
        .then((list) => alive && setResults(list.filter((a) => !a.name.endsWith("(CANCELADO)"))))
        .catch((e) => alive && setError(errorMessage(e)))
        .finally(() => alive && setSearching(false));
    }, 200);
    return () => {
      alive = false;
      window.clearTimeout(id);
    };
  }, [query]);

  const held = view?.positions.filter((p) => Number(p.quantity) !== 0) ?? [];
  const names = view?.names ?? {};

  return (
    <section className="broker">
      <div className="section-head">
        <div>
          <h2>Home broker</h2>
          <p className="muted">
            Procure qualquer ação, FII, ETF, BDR, ação americana ou cripto pelo código ou pelo nome e veja o gráfico, mesmo sem ter o ativo na carteira.
          </p>
        </div>
      </div>

      <div className="card">
        <label className="broker-search">
          <span className="muted">Buscar ativo</span>
          <input
            autoFocus
            type="search"
            placeholder="Ex.: PETR4, IVVB11, AAPL, BTC ou o nome da empresa"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && results.length) onOpen(results[0].ticker);
            }}
          />
        </label>
        {error && <p className="error">{error}</p>}
        {query.trim().length >= 2 && !searching && results.length === 0 && (
          <p className="muted small">
            Nenhum ativo encontrado. A lista de ativos é baixada quando você atualiza as cotações; confira o código ou tente pelo nome.
          </p>
        )}
        {results.length > 0 && (
          <ul className="broker-list">
            {results.map((a) => (
              <li key={a.ticker}>
                <button className="broker-item" onClick={() => onOpen(a.ticker)}>
                  <span>
                    <AssetName asset={a.ticker} name={a.name} />
                  </span>
                  <span className="muted small">{kindLabel(a)}</span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      {held.length > 0 && (
        <>
          <h3 className="spaced">Na sua carteira</h3>
          <div className="broker-chips">
            {held.map((p) => (
              <button key={p.asset} className="chip" onClick={() => onOpen(p.asset)} title={names[p.asset]}>
                {isFund(p.asset) && names[p.asset] ? names[p.asset] : p.asset}
              </button>
            ))}
          </div>
        </>
      )}

      {recent.length > 0 && (
        <>
          <h3 className="spaced">
            Vistos recentemente{" "}
            <button
              className="link small"
              onClick={() => {
                try {
                  localStorage.removeItem(RECENT_KEY);
                } catch {
                  // nada a limpar
                }
                setRecent([]);
              }}
            >
              limpar
            </button>
          </h3>
          <div className="broker-chips">
            {recent.map((r) => (
              <button key={r.ticker} className="chip" onClick={() => onOpen(r.ticker)} title={r.name}>
                {isFund(r.ticker) && r.name ? r.name : r.ticker}
              </button>
            ))}
          </div>
        </>
      )}
    </section>
  );
}
