import { useEffect, useMemo, useState } from "react";
import Intro from "../components/Intro";
import { b3Api, errorMessage, market, notesApi, type Account, type B3Movement, type B3Skipped, type B3Trade, type Operation, type PricedOperation } from "../api";
import { normalizeTicker } from "../currency";
import { date, money, price, quantity } from "../format";
import ImportB3Movements from "./ImportB3Movements";

/** Negócios da mesma corretora no mesmo dia: entram juntos, como uma nota. */
type Group = {
  key: string;
  institution: string;
  date: string;
  trades: B3Trade[];
  hash: string;
  /** Já importado deste extrato antes. */
  imported: boolean;
  /** Já há lançamentos desta conta neste dia (de nota ou manual). */
  existing: "none" | "same" | "different";
  selected: boolean;
  result?: { ok: boolean; text: string };
};

async function sha256(text: string): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(text));
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function plain(s: string): string {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
}

/**
 * Conta para a corretora como a B3 escreve ("ITAU CORRETORA DE VALORES S/A"):
 * a conta já cadastrada cujo nome aparece nela; senão, a primeira palavra.
 */
export function guessAccount(institution: string, accounts: Account[]): string {
  const inst = plain(institution);
  const found = accounts.filter((a) => a.currency === "BRL").find((a) => {
    const first = plain(a.name).split(/\s+/)[0];
    return first.length >= 2 && inst.split(/[^a-z0-9]+/).includes(first);
  });
  if (found) return found.name;
  const word = institution.trim().split(/\s+/)[0] ?? institution;
  return word.charAt(0).toUpperCase() + word.slice(1).toLowerCase();
}

/** Quantidade por ativo e lado, para comparar um dia do extrato com o que já está lançado. */
function signature(items: { asset: string; side: string; quantity: string }[]): string {
  const sum = new Map<string, number>();
  for (const i of items) {
    const k = `${normalizeTicker(i.asset)}|${i.side}`;
    sum.set(k, (sum.get(k) ?? 0) + Number(i.quantity));
  }
  return [...sum.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([k, q]) => `${k}=${q}`)
    .join(";");
}

/** Lançamentos > Importar da B3: os extratos de negociação e de movimentação da Área do Investidor. */
export default function ImportB3({ onImported }: { onImported: () => void }) {
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [ops, setOps] = useState<PricedOperation[]>([]);
  const [file, setFile] = useState<string | null>(null);
  const [groups, setGroups] = useState<Group[]>([]);
  const [skipped, setSkipped] = useState<B3Skipped[]>([]);
  const [mapping, setMapping] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [movements, setMovements] = useState<{ movements: B3Movement[]; skipped: B3Skipped[] } | null>(null);

  useEffect(() => {
    market.listAccounts().then(setAccounts).catch(() => {});
    market.listOperations().then(setOps).catch(() => {});
  }, []);

  async function pick(list: FileList | null) {
    const f = list?.[0];
    if (!f) return;
    setMessage(null);
    setGroups([]);
    setSkipped([]);
    setMovements(null);
    setFile(f.name);
    const bytes = new Uint8Array(await f.arrayBuffer());
    try {
      const x = await b3Api.readTrades(bytes).catch(async (e) => {
        // Extrato de movimentação: proventos e eventos, em outra lista.
        if (!errorMessage(e).includes("movimentação")) throw e;
        setMovements(await b3Api.readMovements(bytes));
        return null;
      });
      if (!x) return;
      setSkipped(x.skipped);
      const map: Record<string, string> = {};
      for (const t of x.trades) map[t.institution] ??= guessAccount(t.institution, accounts);
      setMapping(map);
      const byKey = new Map<string, B3Trade[]>();
      for (const t of x.trades) {
        const key = `${t.institution}|${t.date}`;
        byKey.set(key, [...(byKey.get(key) ?? []), t]);
      }
      const next: Group[] = await Promise.all(
        [...byKey.entries()].map(async ([key, ts]) => {
          const identity = `B3|${ts[0].institution}|${ts[0].date}|` + ts.map((t) => [t.ticker, t.side, t.quantity, t.price].join(",")).join(";");
          return { key, institution: ts[0].institution, date: ts[0].date, trades: ts, hash: await sha256(identity), imported: false, existing: "none", selected: true } as Group;
        }),
      );
      next.sort((a, b) => a.date.localeCompare(b.date) || a.institution.localeCompare(b.institution));
      const status = next.length ? await notesApi.status(next.map((g) => g.hash)).catch(() => []) : [];
      setGroups(next.map((g, i) => ({ ...g, imported: status[i]?.imported ?? false })));
      if (!x.trades.length) setMessage({ ok: false, text: "Nenhuma compra ou venda de ações, FIIs ou ETFs neste extrato." });
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    }
  }

  // Compara cada dia com o que já está lançado na conta escolhida para a corretora.
  const view = useMemo(
    () =>
      groups.map((g) => {
        const account = mapping[g.institution] ?? g.institution;
        const same = ops.filter(
          (p) =>
            p.operation.account.toLowerCase() === account.toLowerCase() &&
            p.operation.date === g.date &&
            (p.operation.side === "COMPRA" || p.operation.side === "VENDA"),
        );
        const existing: Group["existing"] = !same.length
          ? "none"
          : signature(same.map((p) => p.operation)) === signature(g.trades.map((t) => ({ asset: t.ticker, side: t.side, quantity: t.quantity })))
            ? "same"
            : "different";
        return { ...g, account, existing };
      }),
    [groups, mapping, ops],
  );

  const ready = view.filter((g) => !g.imported && !g.result?.ok && g.existing !== "same" && (g.existing === "none" ? g.selected : !g.selected));
  const lines = ready.reduce((n, g) => n + g.trades.length, 0);
  const institutions = Object.keys(mapping);

  async function importAll() {
    setBusy(true);
    setMessage(null);
    let total = 0;
    for (const g of ready) {
      const operations: Operation[] = g.trades.map((t) => ({
        id: "",
        account: g.account.trim(),
        asset: normalizeTicker(t.ticker),
        date: t.date,
        side: t.side,
        quantity: t.quantity,
        price: t.price,
        // Sem taxas no extrato: o aplicativo calcula as da B3 (e a corretagem da conta).
        manual_fees: null,
        note: `Extrato da B3, linha ${t.line}`,
        currency: "BRL",
        from_account: null,
        residue: null,
      }));
      try {
        const n = await notesApi.importNote({ hash: g.hash, broker: "B3", number: null, date: g.date, account: g.account.trim(), operations });
        total += n;
        setGroups((gs) => gs.map((x) => (x.key === g.key ? { ...x, imported: true, result: { ok: true, text: `${n} lançamento(s) importado(s)` } } : x)));
      } catch (e) {
        setGroups((gs) => gs.map((x) => (x.key === g.key ? { ...x, result: { ok: false, text: errorMessage(e) } } : x)));
      }
    }
    setBusy(false);
    if (total) {
      setMessage({ ok: true, text: `${total} lançamento(s) importado(s). Eles aparecem em Lançamentos › Incluir e editar.` });
      market.listAccounts().then(setAccounts).catch(() => {});
      market.listOperations().then(setOps).catch(() => {});
      onImported();
    }
  }

  const toggle = (key: string) => setGroups((gs) => gs.map((g) => (g.key === key ? { ...g, selected: !g.selected } : g)));

  return (
    <section className="import">
      <h2>Importar da B3</h2>
      <Intro
        more={
          <>
            No site da B3, entre na Área do Investidor, abra Extratos › Negociação, escolha o período (pode ser desde a sua primeira compra) e baixe
            em Excel. O extrato traz as compras e vendas de ações, FIIs e ETFs de todas as corretoras; opções, termo e futuro ficam de fora. Ele
            não traz as taxas: o aplicativo calcula as da B3 e a corretagem informada para a conta, como num lançamento manual. Os negócios da
            mesma corretora no mesmo dia entram juntos. Um dia que já tem os mesmos lançamentos (de uma nota de corretagem, por exemplo) fica de
            fora, para não lançar duas vezes.
            <br />
            <br />
            Em Extratos › Movimentação, o extrato traz os dividendos, JCP e rendimentos que caíram na conta, e as bonificações e desdobramentos.
            Escolha esse arquivo aqui também: o aplicativo mostra os proventos para lançar de uma vez (os já lançados ficam de fora) e calcula a
            proporção de cada evento pelas ações que você tinha na data com. As compras e vendas desse extrato ficam de fora: elas vêm do extrato de
            negociação. Os arquivos são lidos aqui no computador.
          </>
        }
      >
        Traga de uma vez o histórico da bolsa, de todas as corretoras: as compras e vendas com o extrato de negociação, e os proventos, bonificações e
        desdobramentos com o extrato de movimentação da Área do Investidor da B3.
      </Intro>
      <div className="actions">
        <label className="file-pick">
          <input
            type="file"
            accept=".xlsx,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
            onChange={(e) => pick(e.target.files).finally(() => (e.target.value = ""))}
          />
          <span className="button">Escolher extrato da B3 (Excel)…</span>
        </label>
      </div>

      {ops.some((p) => p.operation.note === "Posição inicial") && (
        <p className="banner warning small">
          Você já lançou uma posição inicial no primeiro uso. Para não contar as mesmas ações duas vezes, apague essa posição inicial em Lançamentos ›
          Incluir e editar antes de importar o histórico, ou desmarque aqui os dias anteriores a ela.
        </p>
      )}

      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}

      {movements && file && <ImportB3Movements key={file + movements.movements.length} file={file} data={movements} accounts={accounts} onImported={onImported} />}

      {institutions.length > 0 && (
        <div className="card stack">
          <h3>Em qual conta entra cada corretora?</h3>
          <p className="muted small">Use o mesmo nome da conta que você já usa no aplicativo, para os lançamentos ficarem juntos.</p>
          <table className="b3-mapping">
            <thead>
              <tr>
                <th>Corretora no extrato</th>
                <th>Conta no aplicativo</th>
              </tr>
            </thead>
            <tbody>
              {institutions.map((inst) => (
                <tr key={inst}>
                  <td>{inst}</td>
                  <td>
                    <input list="b3-accounts" value={mapping[inst]} onChange={(e) => setMapping((m) => ({ ...m, [inst]: e.target.value }))} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <datalist id="b3-accounts">
            {accounts
              .filter((a) => a.currency === "BRL")
              .map((a) => (
                <option key={a.name} value={a.name} />
              ))}
          </datalist>
        </div>
      )}

      {skipped.length > 0 && (
        <div className="banner warning small">
          <strong>{skipped.length} linha(s) ficam de fora:</strong>
          <ul>
            {skipped.slice(0, 20).map((s) => (
              <li key={s.line}>
                Linha {s.line}: {s.reason} ({s.text})
              </li>
            ))}
            {skipped.length > 20 && <li>e mais {skipped.length - 20}.</li>}
          </ul>
        </div>
      )}

      {view.length > 0 && (
        <>
          <div className="import-summary">
            <p>
              {file}: {view.reduce((n, g) => n + g.trades.length, 0)} negócio(s) em {view.length} dia(s) e corretora(s); {lines} pronto(s) para
              importar.
            </p>
            <button onClick={() => void importAll()} disabled={busy || ready.length === 0 || view.some((g) => !g.account.trim())}>
              {busy ? "Importando…" : `Importar ${lines} lançamento(s)`}
            </button>
          </div>
          {view.map((g) => {
            const willImport = ready.some((r) => r.key === g.key);
            return (
              <div key={g.key} className={`card stack note-card${g.result?.ok || g.imported || g.existing === "same" ? " done" : ""}`}>
                <div className="note-head">
                  <h3>
                    {g.account || g.institution} · {date(g.date)}
                  </h3>
                  {g.result ? (
                    <span className={g.result.ok ? "ok" : "error"}>{g.result.text}</span>
                  ) : g.imported ? (
                    <span className="muted">Já importado deste extrato</span>
                  ) : g.existing === "same" ? (
                    <span className="muted">Já lançado (mesmos negócios na conta)</span>
                  ) : g.existing === "different" ? (
                    <label className="warning-text">
                      <input type="checkbox" checked={willImport} onChange={() => toggle(g.key)} /> Já há outros lançamentos nesta conta neste dia. Importar
                      mesmo assim?
                    </label>
                  ) : (
                    <label className="ok">
                      <input type="checkbox" checked={willImport} onChange={() => toggle(g.key)} /> Importar
                    </label>
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
                        <th className="num">Valor</th>
                        <th className="num">Taxas</th>
                      </tr>
                    </thead>
                    <tbody>
                      {g.trades.map((t) => (
                        <tr key={t.line}>
                          <td className="muted">{t.line}</td>
                          <td>{t.side === "COMPRA" ? "Compra" : "Venda"}</td>
                          <td>{normalizeTicker(t.ticker)}</td>
                          <td className="num">{quantity(t.quantity)}</td>
                          <td className="num">{price(t.price, "BRL")}</td>
                          <td className="num">{money(t.value)}</td>
                          <td className="num">
                            <span className="muted small">automáticas</span>
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>
            );
          })}
        </>
      )}
    </section>
  );
}
