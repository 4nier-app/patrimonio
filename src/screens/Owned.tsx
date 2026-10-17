import { useEffect, useState } from "react";
import Intro from "../components/Intro";
import CashInput from "../components/CashInput";
import { offerUndo } from "../undo";
import { errorMessage, ownedApi, type Improvement, type OwnedAsset, type OwnedKind, type OwnedRow } from "../api";
import { date, money, parseDecimalInput, toCashMask } from "../format";

const today = () => new Date().toLocaleDateString("sv-SE");
const cash = (v: string) => toCashMask(v, 2, "BRL").text;

const KINDS: { id: OwnedKind; title: string; text: string }[] = [
  { id: "IMOVEL", title: "Imóvel", text: "Casa, apartamento, terreno. As obras e reformas somam ao custo." },
  { id: "VEICULO", title: "Veículo", text: "Carro, moto. Entra pelo valor pago na compra." },
];

type Form = { id: string; kind: OwnedKind | null; name: string; bought: string; price: string; sold: string };
const EMPTY = (): Form => ({ id: "", kind: null, name: "", bought: today(), price: "", sold: "" });

type WorkForm = { id: string; ownedId: string; date: string; amount: string; description: string };

/** Bens informados à mão. O valor é o custo (compra + obras), como na ficha Bens e Direitos. */
export default function Owned({ onChanged }: { onChanged: () => void }) {
  const [rows, setRows] = useState<OwnedRow[] | null>(null);
  const [form, setForm] = useState<Form | null>(null);
  const [work, setWork] = useState<WorkForm | null>(null);
  const [confirm, setConfirm] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function load() {
    ownedApi
      .list()
      .then(setRows)
      .catch((e) => setError(errorMessage(e)));
  }
  useEffect(load, []);

  function changed(message: string) {
    setSaved(message || null);
    load();
    onChanged();
  }

  async function run(f: () => Promise<void>) {
    setError(null);
    setBusy(true);
    try {
      await f();
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  function saveOwned(e: React.FormEvent) {
    e.preventDefault();
    if (!form?.kind) return;
    const price = parseDecimalInput(form.price);
    if (price == null || Number(price) <= 0) return setError("Informe o valor pago na compra.");
    const item: OwnedAsset = {
      id: form.id,
      kind: form.kind,
      name: form.name.trim(),
      bought_on: form.bought,
      price,
      sold_on: form.sold || null,
      note: null,
    };
    void run(async () => {
      if (form.id) await ownedApi.update(item);
      else await ownedApi.create(item);
      setForm(null);
      changed(form.id ? "Alteração salva." : `${item.name} cadastrado.`);
    });
  }

  function saveWork(e: React.FormEvent) {
    e.preventDefault();
    if (!work) return;
    const amount = parseDecimalInput(work.amount);
    if (amount == null || Number(amount) <= 0) return setError("Informe o valor da obra.");
    const item: Improvement = { id: work.id, owned_id: work.ownedId, date: work.date, amount, description: work.description.trim() };
    void run(async () => {
      if (work.id) await ownedApi.updateWork(item);
      else await ownedApi.createWork(item);
      setWork(null);
      changed(work.id ? "Obra alterada." : "Obra salva; o valor já soma ao custo do imóvel.");
    });
  }

  function removeOwned(r: OwnedRow) {
    if (confirm !== r.item.id) return setConfirm(r.item.id);
    void run(async () => {
      await ownedApi.remove(r.item.id);
      setConfirm(null);
      changed(`${r.item.name} removido.`);
    });
  }

  function removeWork(w: Improvement) {
    void run(async () => {
      await ownedApi.removeWork(w.id);
      changed("");
      offerUndo("Obra removida.", async () => {
        await ownedApi.createWork({ ...w, id: "" });
        load();
        onChanged();
      });
    });
  }

  function editOwned(a: OwnedAsset) {
    setForm({ id: a.id, kind: a.kind, name: a.name, bought: a.bought_on, price: cash(a.price), sold: a.sold_on ?? "" });
    setSaved(null);
    setError(null);
    window.scrollTo({ top: 0 });
  }

  const items = rows ?? [];
  const active = items.filter((r) => !r.item.sold_on || r.item.sold_on > today());
  const total = active.reduce((s, r) => s + Number(r.cost), 0);

  function workForm(r: OwnedRow) {
    if (!work || work.ownedId !== r.item.id) return null;
    return (
      <form className="savings-move op-form" onSubmit={saveWork}>
        <strong>{work.id ? "Alterar obra" : "Nova obra ou reforma"}</strong>
        <div className="grid">
          <label>
            Data
            <input type="date" value={work.date} min={r.item.bought_on} max={today()} onChange={(e) => setWork({ ...work, date: e.target.value })} required />
          </label>
          <label>
            Valor gasto
            <CashInput value={work.amount} decimals={2} currency="BRL" onChange={(v) => setWork({ ...work, amount: v })} required />
          </label>
          <label>
            Descrição
            <input
              value={work.description}
              placeholder="Ex.: reforma da cozinha"
              onChange={(e) => setWork({ ...work, description: e.target.value })}
              required
            />
          </label>
        </div>
        <p className="muted small">Guarde as notas fiscais e recibos: são eles que comprovam o custo na venda.</p>
        <div className="actions">
          <button type="submit" disabled={busy}>
            {busy ? "Salvando…" : "Salvar"}
          </button>
          <button type="button" className="secondary" onClick={() => setWork(null)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  }

  return (
    <section className="savings">
      <div className="section-head">
        <div>
          <h2>Bens</h2>
          <Intro
            more={
              <>
                O valor de cada bem é o custo: o que você pagou na compra, mais as obras e reformas do imóvel, cada uma a partir da data em que foi
                feita. É assim que a Receita pede em Bens e Direitos. O app não estima valor de mercado. Depois da data de venda, o bem sai do
                patrimônio.
              </>
            }
          >
            Imóveis e veículos, pelo valor pago. Eles entram no patrimônio em Evolução e no Painel.
          </Intro>
        </div>
        {!form && (
          <button type="button" onClick={() => setForm(EMPTY())}>
            + Novo bem
          </button>
        )}
      </div>

      {form && (
        <form className="card op-form" onSubmit={saveOwned}>
          <h3>{form.id ? "Alterar bem" : "1. Que tipo de bem?"}</h3>
          {!form.id && (
            <div className="event-kinds">
              {KINDS.map((k) => (
                <button
                  key={k.id}
                  type="button"
                  className={`event-kind${form.kind === k.id ? " active" : ""}`}
                  onClick={() => setForm({ ...form, kind: k.id })}
                >
                  <strong>{k.title}</strong>
                  <span>{k.text}</span>
                </button>
              ))}
            </div>
          )}
          {form.kind && (
            <>
              {!form.id && <h3 className="spaced">2. Dados da compra</h3>}
              <div className="grid">
                <label>
                  Nome
                  <input
                    value={form.name}
                    placeholder={form.kind === "IMOVEL" ? "Ex.: Apartamento da Rua X" : "Ex.: Carro Onix"}
                    onChange={(e) => setForm({ ...form, name: e.target.value })}
                    required
                  />
                </label>
                <label>
                  Data da compra
                  <input type="date" value={form.bought} max={today()} onChange={(e) => setForm({ ...form, bought: e.target.value })} required />
                </label>
                <label>
                  Valor pago
                  <CashInput value={form.price} decimals={2} currency="BRL" onChange={(v) => setForm({ ...form, price: v })} required />
                </label>
                <label>
                  Data da venda (só se vendeu)
                  <input type="date" value={form.sold} min={form.bought} onChange={(e) => setForm({ ...form, sold: e.target.value })} />
                </label>
              </div>
              {form.kind === "IMOVEL" && !form.id && (
                <p className="muted small">Depois de cadastrar, use "Nova obra" para somar reformas e benfeitorias ao custo.</p>
              )}
            </>
          )}
          {error && <p className="error">{error}</p>}
          <div className="actions">
            {form.kind && (
              <button type="submit" disabled={busy}>
                {busy ? "Salvando…" : form.id ? "Salvar alteração" : "Cadastrar"}
              </button>
            )}
            <button type="button" className="secondary" onClick={() => setForm(null)}>
              Cancelar
            </button>
          </div>
        </form>
      )}

      {!form && error && <p className="error">{error}</p>}
      {saved && <p className="ok">{saved}</p>}

      {active.length > 0 && (
        <div className="savings-totals">
          <div>
            <span className="muted small">Total em bens (custo)</span>
            <strong>{money(String(total))}</strong>
          </div>
        </div>
      )}

      {!rows ? (
        <p className="muted">Carregando…</p>
      ) : items.length === 0 ? (
        !form && <p className="muted">Nenhum bem cadastrado ainda. Clique em "Novo bem" para começar.</p>
      ) : (
        <div className="savings-list">
          {items.map((r) => {
            const a = r.item;
            const house = a.kind === "IMOVEL";
            return (
              <div className="card savings-card" key={a.id}>
                <div className="savings-head">
                  <div>
                    <span className="tag">{house ? "Imóvel" : "Veículo"}</span>
                    <h3>{a.name}</h3>
                    <p className="muted small">
                      Comprado em {date(a.bought_on)} por {money(a.price)}
                      {a.sold_on && ` · vendido em ${date(a.sold_on)}`}
                    </p>
                  </div>
                  <div className="savings-balance">
                    <span className="muted small">{house && r.works.length > 0 ? "Custo (compra + obras)" : "Custo"}</span>
                    <strong>{money(r.cost)}</strong>
                  </div>
                </div>

                {!work || work.ownedId !== a.id ? (
                  <div className="actions">
                    {house && (
                      <button
                        type="button"
                        onClick={() => {
                          setWork({ id: "", ownedId: a.id, date: today(), amount: "", description: "" });
                          setSaved(null);
                          setError(null);
                        }}
                      >
                        Nova obra
                      </button>
                    )}
                    <span className="spacer" />
                    <button type="button" className="link" onClick={() => editOwned(a)}>
                      Alterar
                    </button>
                    <button type="button" className={confirm === a.id ? "danger" : "link subtle"} disabled={busy} onClick={() => removeOwned(r)}>
                      {confirm === a.id ? `Confirmar: remover ${house ? "o imóvel e as obras" : "o veículo"}` : "Remover"}
                    </button>
                  </div>
                ) : (
                  workForm(r)
                )}

                {house && r.works.length > 0 && (
                  <div className="scroll">
                    <table>
                      <thead>
                        <tr>
                          <th>Data</th>
                          <th>Obra</th>
                          <th className="num">Valor</th>
                          <th />
                        </tr>
                      </thead>
                      <tbody>
                        {r.works.map((w) => (
                          <tr key={w.id}>
                            <td>{date(w.date)}</td>
                            <td>{w.description}</td>
                            <td className="num">{money(w.amount)}</td>
                            <td className="row-actions">
                              <button
                                type="button"
                                className="link"
                                onClick={() => setWork({ id: w.id, ownedId: a.id, date: w.date, amount: cash(w.amount), description: w.description })}
                              >
                                Alterar
                              </button>
                              <button type="button" className="link subtle" onClick={() => removeWork(w)}>
                                Remover
                              </button>
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}
