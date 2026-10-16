import { useEffect, useState } from "react";
import Intro from "../components/Intro";
import { offerUndo } from "../undo";
import { errorMessage, eventsApi, type CorporateEvent, type EventKind, type EventRow, type EventsView } from "../api";
import CashInput from "../components/CashInput";
import { date, money, parseDecimalInput, quantity, toCashMask } from "../format";

type Form = {
  id: string;
  kind: EventKind | null;
  asset: string;
  dataCom: string;
  proportion: string;
  cost: string;
};

const EMPTY: Form = { id: "", kind: null, asset: "", dataCom: "", proportion: "", cost: "" };

const KINDS: { id: EventKind; title: string; text: string; example: string }[] = [
  {
    id: "BONIFICACAO",
    title: "Bonificação",
    text: "Ganhei ações novas, sem pagar nada.",
    example: "Ex.: bonificação de 10% (1 ação nova a cada 10)",
  },
  {
    id: "DESDOBRAMENTO",
    title: "Desdobramento",
    text: "Cada ação virou várias, e o preço caiu na mesma proporção.",
    example: "Ex.: desdobramento de 1 para 2",
  },
  {
    id: "GRUPAMENTO",
    title: "Grupamento",
    text: "Várias ações viraram uma, e o preço subiu na mesma proporção.",
    example: "Ex.: grupamento de 10 para 1",
  },
];

function draftOf(f: Form): CorporateEvent | null {
  const proportion = parseDecimalInput(f.proportion);
  if (!f.kind || !f.asset || !f.dataCom || !proportion) return null;
  return {
    id: f.id,
    asset: f.asset,
    kind: f.kind,
    data_com: f.dataCom,
    proportion,
    cost_per_share: f.kind === "BONIFICACAO" ? (parseDecimalInput(f.cost) ?? "0") : "0",
    note: null,
  };
}

/** Número digitado com vírgula: "1,5" ou "10". */
function decimalText(v: string): string {
  return v.replace(/[^\d,]/g, "").replace(/,(?=.*,)/g, "");
}

function signed(v: string): string {
  return `${Number(v) > 0 ? "+" : ""}${quantity(v)}`;
}

export default function Events({ onChanged }: { onChanged: () => void }) {
  const [view, setView] = useState<EventsView | null>(null);
  const [form, setForm] = useState<Form>(EMPTY);
  const [preview, setPreview] = useState<EventRow | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function load() {
    eventsApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }
  useEffect(load, []);

  // Prévia do efeito enquanto o formulário é preenchido.
  const draft = draftOf(form);
  const key = JSON.stringify(draft);
  useEffect(() => {
    setPreviewError(null);
    if (!draft) {
      setPreview(null);
      return;
    }
    let alive = true;
    const t = setTimeout(() => {
      eventsApi
        .preview(draft)
        .then((p) => {
          if (alive) setPreview(p);
        })
        .catch((e) => {
          if (!alive) return;
          setPreview(null);
          setPreviewError(errorMessage(e));
        });
    }, 250);
    return () => {
      alive = false;
      clearTimeout(t);
    };
  }, [key]);

  function set<K extends keyof Form>(k: K, v: Form[K]) {
    setSaved(null);
    setForm((f) => ({ ...f, [k]: v }));
  }

  async function save(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    if (!draft) return;
    setBusy(true);
    try {
      if (form.id) await eventsApi.update(draft);
      else await eventsApi.create(draft);
      setSaved(
        form.id
          ? "Evento alterado. A carteira, o preço médio e os impostos já foram recalculados."
          : "Evento salvo. A carteira, o preço médio e os impostos já consideram a mudança.",
      );
      setForm(EMPTY);
      setPreview(null);
      load();
      onChanged();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function edit(r: EventRow) {
    const ev = r.event;
    setSaved(null);
    setForm({
      id: ev.id,
      kind: ev.kind,
      asset: ev.asset,
      dataCom: ev.data_com,
      proportion: ev.proportion.replace(".", ","),
      cost: Number(ev.cost_per_share) ? toCashMask(ev.cost_per_share, 2, r.currency).text : "",
    });
    window.scrollTo({ top: 0 });
  }

  async function remove(id: string) {
    try {
      await eventsApi.remove(id);
      if (form.id === id) setForm(EMPTY);
      load();
      onChanged();
      offerUndo("Evento removido.", async () => {
        await eventsApi.restore(id);
        load();
        onChanged();
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  const names = view?.names ?? {};
  const currency = preview?.currency ?? "BRL";
  const fractionLost =
    preview != null &&
    preview.currency === "BRL" &&
    form.kind !== "DESDOBRAMENTO" &&
    preview.effects.some((f) => {
      const p = Number(parseDecimalInput(form.proportion) ?? 0);
      const raw = form.kind === "BONIFICACAO" ? Number(f.before) * (1 + p / 100) : Number(f.before) / p;
      return Math.abs(raw - Number(f.after)) > 1e-9;
    });

  return (
    <section className="events">
      <div className="section-head">
        <div>
          <h2>Eventos</h2>
          <Intro more="O aplicativo ajusta a quantidade e o preço médio em todas as contas que tinham o ativo no fim da data com. Remover o evento desfaz o ajuste.">
            Bonificação, desdobramento ou grupamento: lance uma vez por anúncio da empresa.
          </Intro>
        </div>
      </div>

      <form className="card op-form" onSubmit={save}>
        <h3>{form.id ? "Alterar evento" : "1. O que aconteceu?"}</h3>
        <div className="event-kinds" role="radiogroup" aria-label="Tipo de evento">
          {KINDS.map((k) => (
            <button
              key={k.id}
              type="button"
              role="radio"
              aria-checked={form.kind === k.id}
              className={form.kind === k.id ? "event-kind active" : "event-kind"}
              onClick={() => set("kind", k.id)}
            >
              <strong>{k.title}</strong>
              <span>{k.text}</span>
              <span className="muted small">{k.example}</span>
            </button>
          ))}
        </div>

        {form.kind && (
          <>
            <h3 className="spaced">2. Copie os dados do anúncio da empresa</h3>
            <div className="grid">
              <label>
                Ativo
                <select value={form.asset} onChange={(e) => set("asset", e.target.value)} required>
                  <option value="">Escolha o ativo</option>
                  {(view?.assets ?? []).map((a) => (
                    <option key={a} value={a}>
                      {a}
                      {names[a] ? ` · ${names[a]}` : ""}
                    </option>
                  ))}
                </select>
              </label>
              <label>
                Data com
                <input type="date" value={form.dataCom} onChange={(e) => set("dataCom", e.target.value)} required />
                <span className="muted small">Último dia em que quem tinha a ação teve direito ao evento.</span>
              </label>
              {form.kind === "BONIFICACAO" && (
                <>
                  <label>
                    Percentual da bonificação
                    <span className="sentence">
                      <input
                        inputMode="decimal"
                        value={form.proportion}
                        onChange={(e) => set("proportion", decimalText(e.target.value))}
                        placeholder="10"
                        required
                      />
                      % em ações novas
                    </span>
                  </label>
                  <label>
                    Custo por ação nova
                    <CashInput value={form.cost} decimals={2} currency={currency} onChange={(v) => set("cost", v)} />
                    <span className="muted small">Valor que a empresa atribuiu a cada ação bonificada. Se não informou, deixe em branco.</span>
                  </label>
                </>
              )}
              {form.kind === "DESDOBRAMENTO" && (
                <label>
                  Proporção
                  <span className="sentence">
                    Cada 1 ação virou
                    <input
                      inputMode="decimal"
                      value={form.proportion}
                      onChange={(e) => set("proportion", decimalText(e.target.value))}
                      placeholder="2"
                      required
                    />
                    ações
                  </span>
                </label>
              )}
              {form.kind === "GRUPAMENTO" && (
                <label>
                  Proporção
                  <span className="sentence">
                    Cada
                    <input
                      inputMode="decimal"
                      value={form.proportion}
                      onChange={(e) => set("proportion", decimalText(e.target.value))}
                      placeholder="10"
                      required
                    />
                    ações viraram 1 ação
                  </span>
                </label>
              )}
            </div>

            {(preview || previewError) && <h3 className="spaced">3. Confira o resultado</h3>}
            {previewError && <p className="error">{previewError}</p>}
            {preview && preview.effects.length === 0 && (
              <p className="muted">Nenhuma conta tinha {form.asset} no fim de {date(form.dataCom)}. O evento pode ser salvo, mas não muda nada na carteira.</p>
            )}
            {preview && preview.effects.length > 0 && (
              <div className="event-preview">
                <table>
                  <thead>
                    <tr>
                      <th>Conta</th>
                      <th className="num">Antes</th>
                      <th className="num">Depois</th>
                      <th className="num">Diferença</th>
                    </tr>
                  </thead>
                  <tbody>
                    {preview.effects.map((f) => (
                      <tr key={f.account}>
                        <td>{f.account}</td>
                        <td className="num">{quantity(f.before)}</td>
                        <td className="num">
                          <strong>{quantity(f.after)}</strong>
                        </td>
                        <td className="num">{signed(f.delta)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
                <ul className="event-facts">
                  <li>
                    A quantidade muda a partir de <strong>{date(preview.ex_date)}</strong>, o dia seguinte à data com.
                  </li>
                  {preview.average_before && preview.average_after && (
                    <li>
                      Preço médio: {money(preview.average_before, preview.currency)} → <strong>{money(preview.average_after, preview.currency)}</strong>
                    </li>
                  )}
                  {form.kind === "BONIFICACAO" && preview.exempt_brl != null && Number(preview.exempt_brl) > 0 && (
                    <li>
                      Na declaração, informe <strong>{money(preview.exempt_brl)}</strong> em Rendimentos Isentos e Não Tributáveis, na linha
                      "Incorporação de reservas ao capital / Bonificações em ações".
                    </li>
                  )}
                  {form.kind === "BONIFICACAO" && !Number(parseDecimalInput(form.cost) ?? 0) && (
                    <li className="muted">Sem o custo por ação, as ações novas entram com custo zero e o preço médio cai mais.</li>
                  )}
                  {fractionLost && (
                    <li className="muted">
                      Na B3 não existe fração de ação: a empresa vende as sobras em leilão e deposita o dinheiro. Quando ele cair, lance o valor em Proventos
                      com o tipo "Outro".
                    </li>
                  )}
                </ul>
              </div>
            )}
          </>
        )}

        {error && <p className="error">{error}</p>}
        {saved && <p className="ok">{saved}</p>}
        <div className="actions">
          {form.kind && (
            <button type="submit" disabled={busy || !draft || previewError != null}>
              {busy ? "Salvando…" : form.id ? "Salvar alteração" : "Salvar evento"}
            </button>
          )}
          {(form.id || form.kind) && (
            <button
              type="button"
              className="secondary"
              onClick={() => {
                setForm(EMPTY);
                setPreview(null);
              }}
            >
              Cancelar
            </button>
          )}
        </div>
      </form>

      <h3 className="spaced">Eventos lançados</h3>
      {!view ? (
        <p className="muted">Carregando…</p>
      ) : view.items.length === 0 ? (
        <p className="muted">Nenhum evento lançado ainda.</p>
      ) : (
        <div className="scroll">
          <table>
            <thead>
              <tr>
                <th>Data com</th>
                <th>Ativo</th>
                <th>Evento</th>
                <th className="num">Quantidade</th>
                <th className="num">Preço médio</th>
                <th className="num">Isento (declaração)</th>
                <th />
              </tr>
            </thead>
            <tbody>
              {view.items.map((r) => (
                <tr key={r.event.id}>
                  <td>{date(r.event.data_com)}</td>
                  <td>
                    <strong>{r.event.asset}</strong>
                    {names[r.event.asset] && <div className="muted small">{names[r.event.asset]}</div>}
                  </td>
                  <td>{r.label}</td>
                  <td className="num">
                    {quantity(r.before)} → {quantity(r.after)}
                  </td>
                  <td className="num">
                    {r.average_before && r.average_after ? `${money(r.average_before, r.currency)} → ${money(r.average_after, r.currency)}` : "—"}
                  </td>
                  <td className="num">{r.event.kind === "BONIFICACAO" ? (r.exempt_brl != null ? money(r.exempt_brl) : "sem PTAX") : "—"}</td>
                  <td className="row-actions">
                    <button type="button" className="link" onClick={() => edit(r)}>
                      Alterar
                    </button>
                    <button type="button" className="link subtle" onClick={() => remove(r.event.id)}>
                      Remover
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
}
