import { useEffect, useState } from "react";
import { assetApi, errorMessage, market, type Asset, type DayMove, type DayView, type PortfolioView } from "../api";
import { byName } from "../currency";
import { useSort } from "../components/useSort";
import { date, money, tone } from "../format";
import { AssetName } from "./Portfolio";

const pct = (v: number | null) =>
  v == null ? "—" : `${v > 0 ? "+" : ""}${(v * 100).toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}%`;
const amount = (v: number | null, currency: "BRL" | "USD" = "BRL") => (v == null ? "—" : `${v > 0 ? "+" : ""}${money(v.toFixed(2), currency)}`);
const sign = (v: number | null) => (v == null ? "" : tone(String(v)));

function priceText(m: DayMove): string {
  if (m.price == null) return "—";
  const digits = m.price < 1 ? 6 : 2;
  const sym = m.currency === "USD" ? "US$ " : "R$ ";
  return sym + m.price.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: digits });
}

function when(m: DayMove): string {
  if (m.venue === "FUNDO") return m.quote_date ? `cota de ${date(m.quote_date)}` : "sem cota";
  if (m.venue === "TESOURO") return m.quote_date ? `preço de ${date(m.quote_date)}` : "sem preço";
  if (m.offline) return "sem conexão";
  const t = m.price_time ? new Date(m.price_time * 1000).toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" }) : "";
  if (m.venue === "B3") return `${t} · atraso ~15 min`;
  return t;
}

/** Variação de hoje de cada ativo da carteira, atualizada a cada minuto. */
function Today({ refresh, onOpen }: { refresh: number; onOpen: (asset: string) => void }) {
  const [day, setDay] = useState<DayView | null>(null);
  const [loading, setLoading] = useState(true);
  const [at, setAt] = useState<Date | null>(null);

  useEffect(() => {
    let alive = true;
    const load = () => {
      setLoading(true);
      assetApi
        .dayMoves()
        .then((d) => {
          if (!alive) return;
          setDay(d);
          setAt(new Date());
        })
        .catch(() => alive && setDay((d) => d))
        .finally(() => alive && setLoading(false));
    };
    load();
    const id = window.setInterval(() => {
      if (!document.hidden) load();
    }, 60_000);
    return () => {
      alive = false;
      window.clearInterval(id);
    };
  }, [refresh]);

  const items = day?.items ?? [];
  const { sorted, header } = useSort(
    items,
    {
      ativo: (m) => (byName(m.asset) && m.name ? m.name : m.asset),
      variacao: (m) => m.change,
      resultado: (m) => m.result_brl,
      posicao: (m) => m.value_brl,
    },
    "carteira-hoje",
  );

  if (!day && loading) return <p className="muted">Buscando as cotações de agora…</p>;
  if (!day || items.length === 0) return null;
  return (
    <>
      <div className="today-head">
        <h3 className="spaced">Sua carteira hoje</h3>
        <span className="muted small">
          {loading ? "atualizando…" : at ? `atualizado às ${at.toLocaleTimeString("pt-BR", { hour: "2-digit", minute: "2-digit" })}; atualiza a cada minuto` : ""}
        </span>
      </div>
      {day.total_brl != null && (
        <p className="today-total">
          No dia: <strong className={sign(day.total_brl)}>{amount(day.total_brl)}</strong>{" "}
          <span className={sign(day.total_change)}>({pct(day.total_change)})</span>
        </p>
      )}
      <div className="scroll">
        <table className="today-table">
          <thead>
            <tr>
              {header("ativo", "Ativo")}
              <th className="num">Agora</th>
              {header("variacao", "Variação do dia", { numeric: true })}
              {header("resultado", "Ganho/perda no dia", { numeric: true })}
              {header("posicao", "Posição", { numeric: true })}
            </tr>
          </thead>
          <tbody>
            {sorted.map((m) => (
              <tr
                key={m.asset}
                className="clickable go"
                tabIndex={0}
                onClick={() => onOpen(m.asset)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") onOpen(m.asset);
                }}
                title={m.offline ?? "Abrir o gráfico"}
              >
                <td className="asset-link">
                  <AssetName asset={m.asset} name={m.name || undefined} />
                </td>
                <td className="num">
                  {priceText(m)}
                  <div className="muted small">{when(m)}</div>
                </td>
                <td className={`num ${sign(m.change)}`}>{pct(m.change)}</td>
                <td className={`num ${sign(m.result_brl)}`}>
                  {amount(m.result_brl)}
                  {m.currency === "USD" && m.result != null && <div className="muted small">{amount(m.result, "USD")}</div>}
                </td>
                <td className="num">{m.value_brl == null ? "—" : money(m.value_brl.toFixed(2))}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <p className="muted small">
        Ganho ou perda do dia = quantidade de hoje × (preço agora − fechamento de ontem). Ativos em dólar convertidos pela última PTAX
        {day.fx ? ` (R$ ${day.fx.toLocaleString("pt-BR", { minimumFractionDigits: 4 })})` : ""}. Fundos mostram a última cota contra a anterior.
      </p>
    </>
  );
}

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

      {held.length > 0 && <Today refresh={refresh} onOpen={onOpen} />}

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
                {byName(r.ticker) && r.name ? r.name : r.ticker}
              </button>
            ))}
          </div>
        </>
      )}
    </section>
  );
}
