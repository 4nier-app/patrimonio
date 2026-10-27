import { useEffect, useMemo, useState } from "react";
import {
  errorMessage,
  eventsApi,
  incomesApi,
  type Account,
  type B3Movement,
  type B3Skipped,
  type CorporateEvent,
  type EventKind,
  type EventRow,
  type Income,
} from "../api";
import { normalizeTicker } from "../currency";
import { date, money, quantity } from "../format";
import { guessAccount } from "./ImportB3";

type Data = { movements: B3Movement[]; skipped: B3Skipped[] };

const KIND_LABEL: Record<B3Movement["kind"], string> = {
  DIVIDENDO: "Dividendo",
  JCP: "JCP",
  RENDIMENTO: "Rendimento",
  BONIFICACAO: "Bonificação",
  DESDOBRAMENTO: "Desdobramento",
};

/** JCP: o extrato traz o valor já com os 15% de imposto descontados. */
const JCP_TAX = 0.15;

function cents(n: number): string {
  return (Math.round(n * 100) / 100).toFixed(2);
}

function days(a: string, b: string): number {
  return Math.abs(Date.parse(a) - Date.parse(b)) / 86_400_000;
}

function dayBefore(iso: string): string {
  const d = new Date(`${iso}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 1);
  return d.toISOString().slice(0, 10);
}

/**
 * O número mais "redondo" no intervalo [lo, hi): 10 antes de 9,5, 9,5 antes de 9,52.
 * A B3 descarta as frações de ação, então só se sabe que a proporção está nesse intervalo.
 */
function roundest(lo: number, hi: number): string | null {
  if (!(hi > lo)) return null;
  for (let d = 0; d <= 6; d++) {
    const step = 10 ** -d;
    const c = Math.ceil(lo / step - 1e-9) * step;
    if (c < hi - 1e-12) return c.toFixed(d);
  }
  return null;
}

type IncomeLine = {
  key: string;
  movement: B3Movement;
  institution: string;
  asset: string;
  gross: string;
  withheld: string;
  net: string;
  selected: boolean;
  result?: { ok: boolean; text: string };
};

type EventLine = {
  key: string;
  asset: string;
  kind: Exclude<EventKind, "GRUPAMENTO">;
  date: string;
  /** Ações novas creditadas, por corretora. */
  credited: { institution: string; quantity: number }[];
  costPerShare: string;
  dataCom: string;
  proportion: string;
  /** Ações que havia na data com, em todas as contas. */
  heldBefore?: number;
  preview?: EventRow;
  previewError?: string;
  result?: { ok: boolean; text: string };
};

/** Lançamentos > Importar da B3, com o extrato de movimentação: proventos, bonificações e desdobramentos. */
export default function ImportB3Movements({
  file,
  data,
  accounts,
  onImported,
}: {
  file: string;
  data: Data;
  accounts: Account[];
  onImported: () => void;
}) {
  const [existingIncomes, setExistingIncomes] = useState<Income[]>([]);
  const [existingEvents, setExistingEvents] = useState<CorporateEvent[]>([]);
  const [mapping, setMapping] = useState<Record<string, string>>(() => {
    const m: Record<string, string> = {};
    for (const x of data.movements) m[x.institution] ??= guessAccount(x.institution, accounts);
    return m;
  });
  const [incomes, setIncomes] = useState<IncomeLine[]>(() =>
    data.movements
      .filter((m) => m.kind === "DIVIDENDO" || m.kind === "JCP" || m.kind === "RENDIMENTO")
      .map((m) => {
        const net = Number(m.value);
        const gross = m.kind === "JCP" ? net / (1 - JCP_TAX) : net;
        return {
          key: `${m.line}`,
          movement: m,
          institution: m.institution,
          asset: normalizeTicker(m.ticker),
          gross: cents(gross),
          withheld: cents(gross - net),
          net: cents(net),
          selected: true,
        };
      }),
  );
  const [events, setEvents] = useState<EventLine[]>(() => {
    const byKey = new Map<string, EventLine>();
    for (const m of data.movements) {
      if (m.kind !== "BONIFICACAO" && m.kind !== "DESDOBRAMENTO") continue;
      const asset = normalizeTicker(m.ticker);
      const key = `${asset}|${m.kind}|${m.date}`;
      const line = byKey.get(key) ?? {
        key,
        asset,
        kind: m.kind,
        date: m.date,
        credited: [],
        costPerShare: m.kind === "BONIFICACAO" ? Number(m.unit_price ?? 0).toFixed(2).replace(".", ",") : "0",
        dataCom: dayBefore(m.date),
        proportion: "",
      };
      line.credited.push({ institution: m.institution, quantity: Number(m.quantity ?? 0) });
      byKey.set(key, line);
    }
    return [...byKey.values()];
  });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);

  function reload() {
    incomesApi
      .load()
      .then((v) => setExistingIncomes(v.items.map((r) => r.income)))
      .catch(() => {});
    eventsApi
      .load()
      .then((v) => setExistingEvents(v.items.map((r) => r.event)))
      .catch(() => {});
  }
  useEffect(reload, []);

  const accountOf = (institution: string) => (mapping[institution] ?? institution).trim();

  const incomeView = useMemo(
    () =>
      incomes.map((l) => {
        const account = accountOf(l.institution);
        const already = existingIncomes.some(
          (i) =>
            i.asset === l.asset &&
            i.account.toLowerCase() === account.toLowerCase() &&
            days(i.pay_date, l.movement.date) <= 3 &&
            Math.abs(Number(i.gross) - Number(i.withheld) - Number(l.net)) <= 0.05,
        );
        return { ...l, account, already };
      }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [incomes, existingIncomes, mapping],
  );
  const readyIncomes = incomeView.filter((l) => l.selected && !l.already && !l.result?.ok);

  // Quantidade de cada conta na data com, e a proporção que explica as ações creditadas.
  useEffect(() => {
    let cancel = false;
    for (const e of events) {
      if (e.result?.ok || !e.dataCom) continue;
      const probe: CorporateEvent = {
        id: "",
        asset: e.asset,
        kind: e.kind,
        data_com: e.dataCom,
        proportion: e.kind === "BONIFICACAO" ? "10" : "2",
        cost_per_share: e.costPerShare || "0",
        note: null,
      };
      eventsApi
        .preview(probe)
        .then((row) => {
          if (cancel) return;
          const held = new Map(row.effects.map((x) => [x.account.toLowerCase(), Number(x.before)]));
          const totalBefore = row.effects.reduce((n, x) => n + Number(x.before), 0);
          const parts = e.credited.map((c) => ({ before: held.get(accountOf(c.institution).toLowerCase()), added: c.quantity }));
          // Por conta, se as contas batem; senão, pelo total.
          const use = parts.every((p) => p.before) ? (parts as { before: number; added: number }[]) : [{ before: totalBefore, added: e.credited.reduce((n, c) => n + c.quantity, 0) }];
          let lo = 0;
          let hi = Infinity;
          for (const p of use) {
            if (!p.before) continue;
            const scale = e.kind === "BONIFICACAO" ? 100 : 1;
            const base = e.kind === "BONIFICACAO" ? 0 : p.before;
            lo = Math.max(lo, ((base + p.added) / p.before) * scale);
            hi = Math.min(hi, ((base + p.added + 1) / p.before) * scale);
          }
          const proportion = totalBefore > 0 ? (roundest(lo, hi) ?? "") : "";
          setEvents((es) => es.map((x) => (x.key === e.key ? { ...x, heldBefore: totalBefore, proportion: x.proportion || proportion } : x)));
        })
        .catch(() => {});
    }
    return () => {
      cancel = true;
    };
    // Recalcula quando muda a data com ou a conta de uma corretora.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events.map((e) => `${e.key}:${e.dataCom}:${e.proportion === ""}`).join(";"), mapping]);

  // Prévia com a proporção escolhida, para conferir com o extrato.
  useEffect(() => {
    let cancel = false;
    for (const e of events) {
      if (e.result?.ok) continue;
      if (!e.proportion || !e.dataCom) {
        setEvents((es) => es.map((x) => (x.key === e.key && (x.preview || x.previewError) ? { ...x, preview: undefined, previewError: undefined } : x)));
        continue;
      }
      eventsApi
        .preview(toEvent(e))
        .then((row) => !cancel && setEvents((es) => es.map((x) => (x.key === e.key ? { ...x, preview: row, previewError: undefined } : x))))
        .catch((err) => !cancel && setEvents((es) => es.map((x) => (x.key === e.key ? { ...x, preview: undefined, previewError: errorMessage(err) } : x))));
    }
    return () => {
      cancel = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [events.map((e) => `${e.key}:${e.dataCom}:${e.proportion}:${e.costPerShare}`).join(";")]);

  function toEvent(e: EventLine): CorporateEvent {
    return {
      id: "",
      asset: e.asset,
      kind: e.kind,
      data_com: e.dataCom,
      proportion: e.proportion.replace(",", "."),
      cost_per_share: e.kind === "BONIFICACAO" ? (e.costPerShare || "0").replace(",", ".") : "0",
      note: `Extrato de movimentação da B3, crédito em ${date(e.date)}`,
    };
  }

  const eventAlready = (e: EventLine) => existingEvents.some((x) => x.asset === e.asset && x.kind === e.kind && days(x.data_com, e.dataCom) <= 30);

  async function launchIncomes() {
    setBusy(true);
    setMessage(null);
    let total = 0;
    for (const l of readyIncomes) {
      const income: Income = {
        id: "",
        account: l.account,
        asset: l.asset,
        kind: l.movement.kind as Income["kind"],
        pay_date: l.movement.date,
        ex_date: null,
        gross: l.gross,
        withheld: l.withheld,
        currency: "BRL",
        note: `Extrato de movimentação da B3, linha ${l.movement.line}`,
      };
      try {
        await incomesApi.create(income);
        total++;
        setIncomes((ls) => ls.map((x) => (x.key === l.key ? { ...x, result: { ok: true, text: "Lançado" } } : x)));
      } catch (e) {
        setIncomes((ls) => ls.map((x) => (x.key === l.key ? { ...x, result: { ok: false, text: errorMessage(e) } } : x)));
      }
    }
    setBusy(false);
    if (total) {
      setMessage({ ok: true, text: `${total} provento(s) lançado(s). Eles aparecem em Lançamentos › Proventos.` });
      reload();
      onImported();
    }
  }

  async function launchEvent(e: EventLine) {
    try {
      await eventsApi.create(toEvent(e));
      setEvents((es) => es.map((x) => (x.key === e.key ? { ...x, result: { ok: true, text: "Lançado em Eventos" } } : x)));
      reload();
      onImported();
    } catch (err) {
      setEvents((es) => es.map((x) => (x.key === e.key ? { ...x, result: { ok: false, text: errorMessage(err) } } : x)));
    }
  }

  const setEvent = (key: string, patch: Partial<EventLine>) => setEvents((es) => es.map((x) => (x.key === key ? { ...x, ...patch, result: undefined } : x)));
  const institutions = Object.keys(mapping);
  const brl = accounts.filter((a) => a.currency === "BRL");

  return (
    <>
      {message && <p className={message.ok ? "ok" : "error"}>{message.text}</p>}

      {institutions.length > 0 && (
        <div className="card stack">
          <h3>Em qual conta entra cada corretora?</h3>
          <p className="muted small">Escolha a conta onde você lançou as compras desses ativos.</p>
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
                    <input list="b3-mov-accounts" value={mapping[inst]} onChange={(e) => setMapping((m) => ({ ...m, [inst]: e.target.value }))} />
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <datalist id="b3-mov-accounts">
            {brl.map((a) => (
              <option key={a.name} value={a.name} />
            ))}
          </datalist>
        </div>
      )}

      {data.skipped.length > 0 && (
        <details className="banner warning small">
          <summary>
            <strong>{data.skipped.length} linha(s) ficam de fora</strong> (compras e vendas, transferências, frações e outras movimentações)
          </summary>
          <ul>
            {data.skipped.slice(0, 50).map((s) => (
              <li key={s.line}>
                Linha {s.line}: {s.reason} ({s.text})
              </li>
            ))}
            {data.skipped.length > 50 && <li>e mais {data.skipped.length - 50}.</li>}
          </ul>
        </details>
      )}

      {!incomeView.length && !events.length && <p className="muted">{file}: nenhum provento, bonificação ou desdobramento neste extrato.</p>}

      {incomeView.length > 0 && (
        <div className="card stack">
          <div className="note-head">
            <h3>Proventos ({incomeView.length})</h3>
            <button onClick={() => void launchIncomes()} disabled={busy || readyIncomes.length === 0}>
              {busy ? "Lançando…" : `Lançar ${readyIncomes.length} provento(s)`}
            </button>
          </div>
          <p className="muted small">
            O extrato mostra o JCP já com os 15% de imposto descontados: o aplicativo lança o valor bruto e o imposto retido, que somam o que caiu na
            conta. Um provento que já está lançado (mesmo ativo e conta, data e valor próximos) fica de fora.
          </p>
          <div className="scroll">
            <table className="note-lines">
              <thead>
                <tr>
                  <th />
                  <th>Pagamento</th>
                  <th>Conta</th>
                  <th>Ativo</th>
                  <th>Tipo</th>
                  <th className="num">Bruto</th>
                  <th className="num">Imposto</th>
                  <th className="num">Recebido</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {incomeView.map((l) => (
                  <tr key={l.key} className={l.already || l.result?.ok ? "muted" : ""}>
                    <td>
                      {!l.already && !l.result?.ok && (
                        <input
                          type="checkbox"
                          aria-label="Lançar"
                          checked={l.selected}
                          onChange={() => setIncomes((ls) => ls.map((x) => (x.key === l.key ? { ...x, selected: !x.selected } : x)))}
                        />
                      )}
                    </td>
                    <td>{date(l.movement.date)}</td>
                    <td>{l.account}</td>
                    <td>{l.asset}</td>
                    <td>{KIND_LABEL[l.movement.kind]}</td>
                    <td className="num">{money(l.gross)}</td>
                    <td className="num">{money(l.withheld)}</td>
                    <td className="num">{money(l.net)}</td>
                    <td className="small">
                      {l.result ? <span className={l.result.ok ? "ok" : "error"}>{l.result.text}</span> : l.already ? "Já lançado" : null}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {events.map((e) => {
        const credited = e.credited.reduce((n, c) => n + c.quantity, 0);
        const delta = e.preview ? e.preview.effects.reduce((n, x) => n + Number(x.delta), 0) : null;
        const matches = delta != null && Math.abs(delta - credited) < 1e-6;
        const done = e.result?.ok || eventAlready(e);
        return (
          <div key={e.key} className={`card stack note-card${done ? " done" : ""}`}>
            <div className="note-head">
              <h3>
                {KIND_LABEL[e.kind]} de {e.asset} · crédito em {date(e.date)}
              </h3>
              {e.result ? (
                <span className={e.result.ok ? "ok" : "error"}>{e.result.text}</span>
              ) : eventAlready(e) ? (
                <span className="muted">Já lançado em Eventos</span>
              ) : (
                <button onClick={() => void launchEvent(e)} disabled={!e.preview}>
                  Lançar em Eventos
                </button>
              )}
            </div>
            <p className="small">
              A B3 creditou {quantity(String(credited))} ação(ões) nova(s)
              {e.credited.length > 1 ? ` (${e.credited.map((c) => `${quantity(String(c.quantity))} na ${accountOf(c.institution)}`).join(", ")})` : ""}.
            </p>
            {!done && (
              <div className="mov-fields">
                <label>
                  Data com
                  <input type="date" value={e.dataCom} onChange={(x) => setEvent(e.key, { dataCom: x.target.value, proportion: "", heldBefore: undefined })} />
                  <span className="muted small">O extrato traz a data do crédito. Confira a data com no aviso da empresa.</span>
                </label>
                <label>
                  {e.kind === "BONIFICACAO" ? "Bonificação (%)" : "Cada ação vira"}
                  <input inputMode="decimal" value={e.proportion} onChange={(x) => setEvent(e.key, { proportion: x.target.value })} />
                  <span className="muted small">Calculada pelas ações que você tinha na data com e as que a B3 creditou.</span>
                </label>
                {e.kind === "BONIFICACAO" && (
                  <label>
                    Custo por ação nova (R$)
                    <input inputMode="decimal" value={e.costPerShare} onChange={(x) => setEvent(e.key, { costPerShare: x.target.value })} />
                    <span className="muted small">Valor atribuído pela empresa, que vem no extrato.</span>
                  </label>
                )}
              </div>
            )}
            {!done && e.previewError && <p className="error small">{e.previewError}</p>}
            {!done && e.heldBefore === 0 && <p className="warning-text small">Você não tinha {e.asset} na data com informada. Confira a data.</p>}
            {!done && e.preview && (
              <p className={`small ${matches ? "ok" : "warning-text"}`}>
                Com essa proporção, o aplicativo soma {quantity(String(delta))} ação(ões) às {quantity(e.preview.before)} que você tinha
                {matches ? ", o mesmo que o extrato." : `, e o extrato mostra ${quantity(String(credited))}. Confira a data com e a proporção.`}
              </p>
            )}
          </div>
        );
      })}
    </>
  );
}
