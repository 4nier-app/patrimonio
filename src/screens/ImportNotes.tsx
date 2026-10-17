import { useEffect, useMemo, useState } from "react";
import Intro from "../components/Intro";
import { errorMessage, market, notesApi, type Account, type Operation } from "../api";
import { date, money, price, quantity } from "../format";
import { noteHash, readNotes, UnknownLayout } from "../import";
import { PasswordNeeded, pdfPages } from "../import/pdf";
import type { ParsedNote } from "../import/types";

type Item = {
  key: string;
  file: string;
  note: ParsedNote;
  hash: string;
  imported: boolean;
  include: boolean[];
  tickers: string[];
  result?: { ok: boolean; text: string };
};

type FileState = {
  name: string;
  data: Uint8Array;
  account: string;
  status: "lendo" | "senha" | "pronto" | "erro";
  error?: string;
  wrongPassword?: boolean;
};

/** Conta já cadastrada com o nome da corretora, ou o nome da corretora. */
function suggestAccount(broker: string, accounts: Account[]): string {
  const b = broker.toLowerCase();
  return accounts.find((a) => a.name.toLowerCase() === b)?.name ?? accounts.find((a) => a.name.toLowerCase().includes(b))?.name ?? broker;
}

/** Motivo que impede a nota de ser importada; `null` quando pode. */
function blocker(it: Item): string | null {
  if (it.imported) return "Esta nota já foi importada.";
  if (!it.note.check.ok) return "Os totais da nota não conferem.";
  if (!it.include.some(Boolean)) return "Nenhuma linha marcada.";
  if (it.include.some((inc, i) => inc && !it.tickers[i].trim())) return "Falta o código de algum ativo.";
  return null;
}

export default function ImportNotes({ onImported }: { onImported: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [files, setFiles] = useState<FileState[]>([]);
  const [items, setItems] = useState<Item[]>([]);
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    market.listAccounts().then(setAccounts).catch(() => {});
  }, []);

  async function load(f: FileState, pw?: string) {
    setFiles((fs) => fs.map((x) => (x.name === f.name ? { ...x, status: "lendo", error: undefined } : x)));
    try {
      const pages = await pdfPages(f.data.slice(), pw || undefined);
      const notes = readNotes(pages);
      const hashes = await Promise.all(notes.map(noteHash));
      const status = await notesApi.status(hashes);
      // Nome de pregão → código, para as notas que não trazem o código.
      const lookups = notes.flatMap((n) => n.lines.filter((l) => !l.ticker && l.lookup).map((l) => l.lookup!));
      const found = lookups.length
        ? await notesApi.resolveNames(lookups.map((l) => [l.name, l.cls] as [string, string]))
        : [];
      let k = 0;
      const next: Item[] = notes.map((note, i) => {
        const tickers = note.lines.map((l) => {
          if (l.ticker || !l.lookup) return l.ticker ?? "";
          const c = found[k++] ?? [];
          if (c.length > 1) l.candidates = c;
          return c.length === 1 ? c[0] : "";
        });
        return {
          key: `${f.name}#${i}`,
          file: f.name,
          note,
          hash: hashes[i],
          imported: status[i]?.imported ?? false,
          include: note.lines.map((l) => !l.skip),
          tickers,
        };
      });
      const account = suggestAccount(notes[0]?.broker ?? "", accounts);
      setItems((its) => [...its.filter((x) => x.file !== f.name), ...next]);
      setFiles((fs) => fs.map((x) => (x.name === f.name ? { ...x, status: "pronto", account: x.account || account } : x)));
    } catch (e) {
      if (e instanceof PasswordNeeded) {
        setFiles((fs) => fs.map((x) => (x.name === f.name ? { ...x, status: "senha", wrongPassword: e.wrong } : x)));
      } else {
        const text = e instanceof UnknownLayout ? e.message : `Não consegui ler o PDF: ${errorMessage(e)}`;
        setFiles((fs) => fs.map((x) => (x.name === f.name ? { ...x, status: "erro", error: text } : x)));
      }
    }
  }

  async function pick(list: FileList | null) {
    if (!list) return;
    setMessage(null);
    const added: FileState[] = [];
    for (const file of Array.from(list)) {
      const data = new Uint8Array(await file.arrayBuffer());
      added.push({ name: file.name, data, account: "", status: "lendo" });
    }
    setFiles((fs) => [...fs.filter((x) => !added.some((a) => a.name === x.name)), ...added]);
    for (const f of added) await load(f, password);
  }

  function update(key: string, fn: (it: Item) => Item) {
    setItems((its) => its.map((x) => (x.key === key ? fn(x) : x)));
  }

  const ready = useMemo(() => items.filter((it) => !blocker(it) && !it.result?.ok), [items]);

  async function importAll() {
    setBusy(true);
    setMessage(null);
    let notes = 0;
    let ops = 0;
    for (const it of ready) {
      const account = files.find((f) => f.name === it.file)?.account.trim() || it.note.broker;
      const operations: Operation[] = it.note.lines
        .map((l, i) => ({ l, i }))
        .filter(({ i }) => it.include[i])
        .map(({ l, i }) => ({
          id: "",
          account,
          asset: it.tickers[i].trim().toUpperCase(),
          date: it.note.date,
          side: l.side,
          quantity: l.quantity,
          price: l.price,
          manual_fees: l.fees,
          note: `${it.note.number ? `Nota ${it.note.number}` : "Confirmação"} de ${date(it.note.date)} (${it.note.broker}): ${l.title}`,
          currency: it.note.currency,
          from_account: null,
          residue: null,
        }));
      try {
        const n = await notesApi.importNote({
          hash: it.hash,
          broker: it.note.broker,
          number: it.note.number,
          date: it.note.date,
          account,
          operations,
        });
        notes++;
        ops += n;
        update(it.key, (x) => ({ ...x, imported: true, result: { ok: true, text: `Importada: ${n} lançamento(s).` } }));
      } catch (e) {
        update(it.key, (x) => ({ ...x, result: { ok: false, text: errorMessage(e) } }));
      }
    }
    setBusy(false);
    if (notes) {
      setMessage({ ok: true, text: `${notes} nota(s) importada(s), com ${ops} lançamento(s).` });
      market.listAccounts().then(setAccounts).catch(() => {});
      onImported();
    }
  }

  return (
    <section className="import">
      <h2>Importar notas de corretagem</h2>
      <Intro more="Aceita notas da Itaú, Nubank e outras no padrão da B3, e confirmações da Nomad. Antes de gravar, o aplicativo confere os totais de cada nota e não importa a mesma nota duas vezes.">
        Escolha um ou mais PDFs de notas. Eles são lidos aqui no computador e não vão para lugar nenhum.
      </Intro>
      <label className="file-pick">
        <input type="file" accept="application/pdf,.pdf" multiple onChange={(e) => pick(e.target.files).finally(() => (e.target.value = ""))} />
        <span className="button">Escolher PDFs…</span>
      </label>

      {files.map((f) => (
        <div key={f.name} className="card stack import-file">
          <div className="import-file-head">
            <strong>{f.name}</strong>
            {f.status === "lendo" && <span className="muted">Lendo…</span>}
            {f.status === "pronto" && (
              <label className="inline">
                Conta
                <input list="contas-import" value={f.account} onChange={(e) => setFiles((fs) => fs.map((x) => (x.name === f.name ? { ...x, account: e.target.value } : x)))} />
              </label>
            )}
          </div>
          {f.status === "erro" && <p className="error">{f.error}</p>}
          {f.status === "senha" && (
            <form
              className="actions"
              onSubmit={(e) => {
                e.preventDefault();
                load(f, password);
              }}
            >
              <span className={f.wrongPassword ? "error" : "muted"}>
                {f.wrongPassword ? "Senha incorreta." : "Este PDF tem senha (muitas corretoras usam o CPF, só números)."}
              </span>
              <input type="password" value={password} onChange={(e) => setPassword(e.target.value)} placeholder="Senha do PDF" autoFocus />
              <button type="submit">Abrir</button>
            </form>
          )}
        </div>
      ))}
      <datalist id="contas-import">
        {accounts.map((a) => (
          <option key={a.name} value={a.name} />
        ))}
      </datalist>

      {items.length > 0 && (
        <>
          <div className="import-summary">
            <p>
              {items.length} nota(s) encontrada(s); {ready.length} pronta(s) para importar.
            </p>
            <button onClick={importAll} disabled={busy || ready.length === 0}>
              {busy ? "Importando…" : `Importar ${ready.length} nota(s)`}
            </button>
          </div>
          {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}
          {items.map((it) => (
            <NoteCard key={it.key} it={it} onChange={(fn) => update(it.key, fn)} />
          ))}
        </>
      )}
    </section>
  );
}

function NoteCard({ it, onChange }: { it: Item; onChange: (fn: (it: Item) => Item) => void }) {
  const n = it.note;
  const cur = n.currency;
  const why = blocker(it);
  return (
    <div className={`card stack note-card${it.result?.ok ? " done" : ""}`}>
      <div className="note-head">
        <h3>
          {n.broker} · {date(n.date)}
          {n.number ? ` · nota ${n.number}` : ""}
        </h3>
        {it.result ? (
          <span className={it.result.ok ? "ok" : "error"}>{it.result.text}</span>
        ) : why ? (
          <span className={it.imported ? "muted" : "warning-text"}>{why}</span>
        ) : (
          <span className="ok">Pronta</span>
        )}
      </div>
      <p className={n.check.ok ? "muted small" : "error"}>{n.check.text}</p>
      {n.warnings.map((w) => (
        <p key={w} className="warning small">
          {w}
        </p>
      ))}
      <div className="scroll">
        <table className="note-lines">
          <thead>
            <tr>
              <th></th>
              <th>C/V</th>
              <th>Ativo</th>
              <th>Na nota</th>
              <th className="num">Quantidade</th>
              <th className="num">Preço</th>
              <th className="num">Valor</th>
              <th className="num">Taxas</th>
            </tr>
          </thead>
          <tbody>
            {n.lines.map((l, i) => (
              <tr key={i} className={it.include[i] ? "" : "off"}>
                <td>
                  <input
                    type="checkbox"
                    aria-label="Importar esta linha"
                    checked={it.include[i]}
                    disabled={it.imported}
                    onChange={(e) => onChange((x) => ({ ...x, include: x.include.map((v, j) => (j === i ? e.target.checked : v)) }))}
                  />
                </td>
                <td>{l.side === "COMPRA" ? "Compra" : "Venda"}</td>
                <td>
                  {l.candidates && l.candidates.length > 1 && !it.imported ? (
                    <select value={it.tickers[i]} onChange={(e) => onChange((x) => ({ ...x, tickers: x.tickers.map((v, j) => (j === i ? e.target.value : v)) }))}>
                      <option value="">Escolha…</option>
                      {l.candidates.map((c) => (
                        <option key={c}>{c}</option>
                      ))}
                    </select>
                  ) : (
                    <input
                      className={`ticker-input${it.include[i] && !it.tickers[i] ? " missing" : ""}`}
                      value={it.tickers[i]}
                      disabled={it.imported}
                      placeholder="Código"
                      onChange={(e) => onChange((x) => ({ ...x, tickers: x.tickers.map((v, j) => (j === i ? e.target.value.toUpperCase() : v)) }))}
                    />
                  )}
                </td>
                <td className="muted small">
                  {l.title}
                  {l.market === "FRACIONARIO" ? " · fracionário" : l.market !== "VISTA" && l.market !== "EUA" ? ` · ${l.market.toLowerCase()}` : ""}
                  {l.dayTrade ? " · day trade" : ""}
                  {l.skip && <div className="warning-text">{l.skip}</div>}
                </td>
                <td className="num">{quantity(l.quantity)}</td>
                <td className="num">{price(l.price, cur)}</td>
                <td className="num">{money(l.value, cur)}</td>
                <td className="num">{money(l.fees, cur)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {n.fees.length > 0 && (
        <p className="muted small">
          Taxas da nota: {n.fees.map((f) => `${f.label} ${money(f.amount, cur)}`).join(" · ")}
          {n.reader === "sinacor"
            ? ` · total ${money(n.totalFees, cur)}, rateado entre as linhas pelo valor de cada uma.`
            : ". Cada ordem leva as próprias taxas."}
        </p>
      )}
    </div>
  );
}
