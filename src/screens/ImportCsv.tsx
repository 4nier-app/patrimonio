import { useEffect, useMemo, useState } from "react";
import Intro from "../components/Intro";
import { errorMessage, market, notesApi, type Account, type Operation } from "../api";
import { currencyOfNewAccount, fundTicker, normalizeTicker } from "../currency";
import { date, money, price, quantity } from "../format";
import { CSV_TEMPLATE, parseCsv, type CsvError, type CsvRow } from "../import/csv";

/** Lançamentos da mesma conta no mesmo dia: entram juntos, como uma nota. */
type Group = {
  key: string;
  account: string;
  date: string;
  currency: "BRL" | "USD";
  rows: CsvRow[];
  hash: string;
  imported: boolean;
  problem: string | null;
  result?: { ok: boolean; text: string };
};

/** Excel em português grava em Windows-1252; os outros, em UTF-8. */
function decode(bytes: ArrayBuffer): string {
  try {
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    return new TextDecoder("windows-1252").decode(bytes);
  }
}

async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

const tickerOf = (asset: string) => fundTicker(asset) ?? normalizeTicker(asset);

/** Contas das linhas de exemplo de CSV_TEMPLATE. */
const EXAMPLE_ACCOUNTS = ["corretora exemplo", "exterior exemplo"];

export default function ImportCsv({ onImported }: { onImported: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [file, setFile] = useState<string | null>(null);
  const [groups, setGroups] = useState<Group[]>([]);
  const [errors, setErrors] = useState<CsvError[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    market.listAccounts().then(setAccounts).catch(() => {});
  }, []);

  async function pick(list: FileList | null) {
    const f = list?.[0];
    if (!f) return;
    setMessage(null);
    setFile(f.name);
    const { rows, errors } = parseCsv(decode(await f.arrayBuffer()));
    setErrors(errors);
    const byKey = new Map<string, CsvRow[]>();
    for (const r of rows) {
      const key = `${r.account.toLowerCase()}|${r.date}`;
      byKey.set(key, [...(byKey.get(key) ?? []), r]);
    }
    const next: Group[] = await Promise.all(
      [...byKey.entries()].map(async ([key, rs]) => {
        const account = accounts.find((a) => a.name.toLowerCase() === rs[0].account.toLowerCase())?.name ?? rs[0].account;
        const known = accounts.find((a) => a.name === account)?.currency;
        const currencies = new Set(rs.map((r) => r.currency ?? known ?? currencyOfNewAccount(account, tickerOf(r.asset), [])));
        const currency = (known ?? [...currencies][0]) as "BRL" | "USD";
        const problem =
          currencies.size > 1 || (known && !currencies.has(known))
            ? `A conta ${account} é em ${known === "USD" ? "dólar" : "reais"}, mas há linhas em outra moeda.`
            : null;
        const identity = `CSV|${account.toLowerCase()}|${rs[0].date}|` + rs.map((r) => [r.asset, r.side, r.quantity, r.price, r.fees ?? ""].join(",")).join(";");
        return { key, account, date: rs[0].date, currency, rows: rs, hash: await sha256(identity), imported: false, problem };
      }),
    );
    next.sort((a, b) => a.date.localeCompare(b.date) || a.account.localeCompare(b.account));
    const status = next.length ? await notesApi.status(next.map((g) => g.hash)).catch(() => []) : [];
    setGroups(next.map((g, i) => ({ ...g, imported: status[i]?.imported ?? false })));
  }

  const ready = useMemo(() => groups.filter((g) => !g.imported && !g.problem && !g.result?.ok), [groups]);
  const lines = ready.reduce((n, g) => n + g.rows.length, 0);

  async function importAll() {
    setBusy(true);
    setMessage(null);
    let ops = 0;
    for (const g of ready) {
      const operations: Operation[] = g.rows.map((r) => ({
        id: "",
        account: g.account,
        asset: tickerOf(r.asset),
        date: r.date,
        side: r.side,
        quantity: r.quantity,
        price: r.price,
        manual_fees: r.fees,
        note: r.note ?? `Planilha ${file ?? ""}, linha ${r.line}`.trim(),
        currency: g.currency,
        from_account: null,
        residue: null,
        ir: r.ir,
        iof: r.iof,
      }));
      try {
        const n = await notesApi.importNote({ hash: g.hash, broker: "Planilha", number: null, date: g.date, account: g.account, operations });
        ops += n;
        setGroups((gs) => gs.map((x) => (x.key === g.key ? { ...x, imported: true, result: { ok: true, text: `${n} lançamento(s) importado(s)` } } : x)));
      } catch (e) {
        setGroups((gs) => gs.map((x) => (x.key === g.key ? { ...x, result: { ok: false, text: errorMessage(e) } } : x)));
      }
    }
    setBusy(false);
    if (ops) {
      setMessage({ ok: true, text: `${ops} lançamento(s) importado(s). Eles aparecem em Lançamentos › Incluir e editar.` });
      market.listAccounts().then(setAccounts).catch(() => {});
      onImported();
    }
  }

  async function template() {
    try {
      if (await notesApi.saveCsvTemplate(CSV_TEMPLATE)) setMessage({ ok: true, text: "Modelo salvo. Abra no Excel, apague as linhas de exemplo, preencha com os seus lançamentos e salve como CSV." });
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    }
  }

  return (
    <section className="import">
      <h2>Importar planilha (CSV)</h2>
      <Intro
        more={
          <>
            Uma linha por compra ou venda, com as colunas data, conta, ativo, operacao (Compra ou Venda), quantidade e preco. As colunas taxas,
            moeda (BRL ou USD), ir, iof e observacao são opcionais: sem taxas, o aplicativo calcula as da B3. Fundos entram pelo CNPJ no lugar do
            código. Os lançamentos da mesma conta no mesmo dia entram juntos, e a mesma planilha não é importada duas vezes.
          </>
        }
      >
        Para corretoras sem leitor de nota ou para um histórico guardado em planilha. O arquivo é lido aqui no computador.
      </Intro>
      <div className="actions">
        <label className="file-pick">
          <input type="file" accept=".csv,text/csv,text/plain" onChange={(e) => pick(e.target.files).finally(() => (e.target.value = ""))} />
          <span className="button">Escolher planilha…</span>
        </label>
        <button type="button" className="secondary" onClick={() => void template()}>
          Baixar modelo
        </button>
      </div>
      <p className="muted small">No modelo, apague as linhas de exemplo antes de importar.</p>

      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}

      {errors.length > 0 && (
        <div className="banner warning small">
          <strong>
            {errors.length} linha(s) com problema{groups.length ? " (as outras podem ser importadas)" : ""}:
          </strong>
          <ul>
            {errors.slice(0, 20).map((e) => (
              <li key={e.line}>
                Linha {e.line}: {e.message}
              </li>
            ))}
            {errors.length > 20 && <li>e mais {errors.length - 20}.</li>}
          </ul>
        </div>
      )}

      {groups.some((g) => EXAMPLE_ACCOUNTS.includes(g.account.toLowerCase())) && (
        <p className="banner warning small">
          Esta planilha ainda tem as linhas de exemplo do modelo (contas "Corretora Exemplo" ou "Exterior Exemplo"). Apague essas linhas antes de
          importar.
        </p>
      )}

      {groups.length > 0 && (
        <>
          <div className="import-summary">
            <p>
              {file}: {groups.reduce((n, g) => n + g.rows.length, 0)} lançamento(s) em {groups.length} dia(s) e conta(s); {lines} pronto(s) para
              importar.
            </p>
            <button onClick={() => void importAll()} disabled={busy || ready.length === 0}>
              {busy ? "Importando…" : `Importar ${lines} lançamento(s)`}
            </button>
          </div>
          {groups.map((g) => (
            <div key={g.key} className={`card stack note-card${g.result?.ok ? " done" : ""}`}>
              <div className="note-head">
                <h3>
                  {g.account} · {date(g.date)}
                </h3>
                {g.result ? (
                  <span className={g.result.ok ? "ok" : "error"}>{g.result.text}</span>
                ) : g.imported ? (
                  <span className="muted">Já importado</span>
                ) : g.problem ? (
                  <span className="warning-text">{g.problem}</span>
                ) : (
                  <span className="ok">Pronto</span>
                )}
              </div>
              <div className="scroll">
                <table className="note-lines">
                  <thead>
                    <tr>
                      <th>Linha</th>
                      <th>C/V</th>
                      <th>Ativo</th>
                      <th className="num">Quantidade</th>
                      <th className="num">Preço</th>
                      <th className="num">Taxas</th>
                    </tr>
                  </thead>
                  <tbody>
                    {g.rows.map((r) => (
                      <tr key={r.line}>
                        <td className="muted">{r.line}</td>
                        <td>{r.side === "COMPRA" ? "Compra" : "Venda"}</td>
                        <td>{tickerOf(r.asset)}</td>
                        <td className="num">{quantity(r.quantity)}</td>
                        <td className="num">{price(r.price, g.currency)}</td>
                        <td className="num">{r.fees == null ? <span className="muted small">automáticas</span> : money(r.fees, g.currency)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          ))}
        </>
      )}
    </section>
  );
}
