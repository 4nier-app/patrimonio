import { useEffect, useState } from "react";
import Intro from "../components/Intro";
import CashInput from "../components/CashInput";
import { offerUndo } from "../undo";
import {
  errorMessage,
  market,
  savingsApi,
  type Account,
  type Savings as SavingsT,
  type SavingsItem,
  type SavingsKind,
  type SavingsMove,
  type SavingsMoveKind,
  type SavingsView,
} from "../api";
import { date, money, parseDecimalInput, toCashMask, tone } from "../format";

const today = () => new Date().toLocaleDateString("sv-SE");

const KINDS: { id: SavingsKind; title: string; text: string; example: string }[] = [
  { id: "COFRINHO", title: "Cofrinho", text: "Caixinha ou conta que rende, com resgate a qualquer hora.", example: "Ex.: Nubank, Mercado Pago, 99Pay" },
  { id: "FGTS", title: "FGTS", text: "Saldo do FGTS, como aparece no app do FGTS.", example: "Rende 3% ao ano + TR + lucros" },
  { id: "CONTA", title: "Conta corrente", text: "Dinheiro parado na conta, sem rendimento.", example: "Ex.: Itaú, Bradesco" },
  { id: "BENEFICIO", title: "Cartão benefício", text: "Saldo de vale-refeição ou alimentação.", example: "Ex.: Swile, Alelo, VR" },
];

const ISSUERS: Record<SavingsKind, string[]> = {
  COFRINHO: ["Nubank", "Mercado Pago", "99Pay", "PicPay", "Inter", "C6 Bank", "PagBank", "Itaú"],
  FGTS: ["Caixa"],
  CONTA: ["Itaú", "Bradesco", "Banco do Brasil", "Caixa", "Santander", "Nubank", "Inter"],
  BENEFICIO: ["Swile", "Alelo", "VR", "Ticket", "Pluxee", "Caju", "Flash"],
};

const NAME_HINT: Record<SavingsKind, string> = {
  COFRINHO: "Ex.: Reserva de emergência",
  FGTS: "Ex.: FGTS",
  CONTA: "Ex.: Conta corrente",
  BENEFICIO: "Ex.: Refeição",
};

const MOVE_LABEL: Record<SavingsMoveKind, string> = { DEPOSITO: "Depósito", RESGATE: "Resgate", SALDO: "Saldo informado" };

type NewForm = { id: string; kind: SavingsKind | null; account: string; name: string; rate: string; taxFree: boolean };
const EMPTY_NEW: NewForm = { id: "", kind: null, account: "", name: "", rate: "100", taxFree: false };

type MoveForm = { id: string; savingsId: string; kind: SavingsMoveKind; date: string; amount: string; ir: string; iof: string };

const cash = (v: string) => toCashMask(v, 2, "BRL").text;

export default function Savings({ onChanged }: { onChanged: () => void }) {
  const [view, setView] = useState<SavingsView | null>(null);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [form, setForm] = useState<NewForm | null>(null);
  const [move, setMove] = useState<MoveForm | null>(null);
  const [lastDate, setLastDate] = useState(today());
  const [open, setOpen] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function load() {
    savingsApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }

  useEffect(load, []);
  useEffect(() => {
    market.listAccounts().then(setAccounts).catch(() => setAccounts([]));
  }, []);

  function changed(message: string) {
    setSaved(message);
    load();
    onChanged();
  }

  async function saveSavings(e: React.FormEvent) {
    e.preventDefault();
    if (!form?.kind) return;
    setError(null);
    const item: SavingsT = {
      id: form.id,
      kind: form.kind,
      account: form.account.trim(),
      name: form.name.trim() || KINDS.find((k) => k.id === form.kind)!.title,
      rate_pct: parseDecimalInput(form.rate) ?? "100",
      tax_free: form.taxFree,
      note: null,
    };
    setBusy(true);
    try {
      if (form.id) {
        await savingsApi.update(item);
        changed("Alteração salva.");
      } else {
        await savingsApi.create(item);
        changed(`${item.name} cadastrado. Informe o saldo de cada mês em Evolução › Patrimônio, clicando na célula.`);
      }
      setForm(null);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function saveMove(e: React.FormEvent) {
    e.preventDefault();
    if (!move) return;
    setError(null);
    const amount = parseDecimalInput(move.amount);
    if (amount == null || (move.kind !== "SALDO" && Number(amount) <= 0)) return setError("Informe o valor.");
    const item: SavingsMove = {
      id: move.id,
      savings_id: move.savingsId,
      date: move.date,
      kind: move.kind,
      amount,
      ir: move.kind === "RESGATE" ? parseDecimalInput(move.ir) : null,
      iof: move.kind === "RESGATE" ? parseDecimalInput(move.iof) : null,
      note: null,
    };
    setBusy(true);
    try {
      if (move.id) await savingsApi.updateMove(item);
      else await savingsApi.createMove(item);
      setLastDate(move.date);
      setMove(null);
      setOpen((o) => ({ ...o, [move.savingsId]: true }));
      changed(move.id ? "Movimento alterado." : `${MOVE_LABEL[move.kind]} salvo.`);
    } catch (err) {
      setError(errorMessage(err));
    } finally {
      setBusy(false);
    }
  }

  async function removeSavings(it: SavingsItem) {
    try {
      await savingsApi.remove(it.savings.id);
      changed("");
      setSaved(null);
      offerUndo(`${it.savings.name} removido.`, async () => {
        await savingsApi.restore(it.savings.id);
        load();
        onChanged();
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  async function removeMove(m: SavingsMove) {
    try {
      await savingsApi.removeMove(m.id);
      changed("");
      setSaved(null);
      offerUndo("Movimento removido.", async () => {
        await savingsApi.restoreMove(m.id);
        load();
        onChanged();
      });
    } catch (err) {
      setError(errorMessage(err));
    }
  }

  function startMove(it: SavingsItem, kind: SavingsMoveKind) {
    setMove({ id: "", savingsId: it.savings.id, kind, date: kind === "SALDO" ? today() : lastDate, amount: "", ir: "", iof: "" });
    setSaved(null);
    setError(null);
  }

  function editMove(m: SavingsMove) {
    setMove({
      id: m.id,
      savingsId: m.savings_id,
      kind: m.kind,
      date: m.date,
      amount: cash(m.amount),
      ir: m.ir != null ? cash(m.ir) : "",
      iof: m.iof != null ? cash(m.iof) : "",
    });
    setSaved(null);
    setError(null);
  }

  function editSavings(s: SavingsT) {
    setForm({ id: s.id, kind: s.kind, account: s.account, name: s.name, rate: s.rate_pct, taxFree: s.tax_free });
    setSaved(null);
    window.scrollTo({ top: 0 });
  }

  const items = view?.items ?? [];
  const accountNames = [...new Set([...(form?.kind ? ISSUERS[form.kind] : []), ...accounts.map((a) => a.name)])];

  function moveForm(it: SavingsItem) {
    if (!move || move.savingsId !== it.savings.id) return null;
    return (
      <form className="savings-move op-form" onSubmit={saveMove}>
        <strong>{move.id ? `Alterar ${MOVE_LABEL[move.kind].toLowerCase()}` : MOVE_LABEL[move.kind]}</strong>
        <div className="grid">
          {move.id && it.savings.kind === "COFRINHO" && (
            <label>
              Tipo
              <select value={move.kind} onChange={(e) => setMove({ ...move, kind: e.target.value as SavingsMoveKind })}>
                {(Object.keys(MOVE_LABEL) as SavingsMoveKind[]).map((k) => (
                  <option key={k} value={k}>
                    {MOVE_LABEL[k]}
                  </option>
                ))}
              </select>
            </label>
          )}
          <label>
            Data
            <input type="date" value={move.date} max={today()} onChange={(e) => setMove({ ...move, date: e.target.value })} required />
          </label>
          <label>
            {move.kind === "SALDO" ? "Saldo que o app do banco mostra" : move.kind === "DEPOSITO" ? "Valor depositado" : "Valor que caiu na conta"}
            <CashInput value={move.amount} decimals={2} currency="BRL" onChange={(v) => setMove({ ...move, amount: v })} required />
          </label>
          {move.kind === "RESGATE" && it.savings.kind === "COFRINHO" && (
            <>
              <label>
                IR retido (opcional)
                <CashInput value={move.ir} decimals={2} currency="BRL" onChange={(v) => setMove({ ...move, ir: v })} />
              </label>
              <label>
                IOF retido (opcional)
                <CashInput value={move.iof} decimals={2} currency="BRL" onChange={(v) => setMove({ ...move, iof: v })} />
              </label>
            </>
          )}
        </div>
        {move.kind === "RESGATE" && it.savings.kind === "COFRINHO" && (
          <p className="muted small">Se o banco não mostrar o IR e o IOF, deixe em branco.</p>
        )}
        <div className="actions">
          <button type="submit" disabled={busy}>
            {busy ? "Salvando…" : "Salvar"}
          </button>
          <button type="button" className="secondary" onClick={() => setMove(null)}>
            Cancelar
          </button>
        </div>
      </form>
    );
  }

  function detail(it: SavingsItem, m: SavingsMove): string {
    if (m.kind === "SALDO") {
      const c = it.checks.find((x) => x.move_id === m.id);
      if (!c) return "saldo inicial";
      const d = Number(c.difference);
      if (Math.abs(d) < 0.005) return "igual ao esperado";
      return `esperado ${money(c.expected)}, diferença ${d > 0 ? "+" : ""}${money(c.difference)}`;
    }
    if (m.kind === "RESGATE") {
      const r = it.redemptions.find((x) => x.move_id === m.id);
      if (!r || (Number(r.ir) === 0 && Number(r.iof) === 0)) return "";
      return `bruto ${money(r.gross)} · IR ${money(r.ir)} · IOF ${money(r.iof)}`;
    }
    return "";
  }

  return (
    <section className="savings">
      <div className="section-head">
        <div>
          <h2>Saldos e reservas</h2>
          <Intro
            more={
              <>
                O que vale é o saldo que o banco mostra: informe o saldo de cada mês clicando no mês, em Evolução › Patrimônio. O app não estima nada
                entre um saldo e outro. No cofrinho, lance também os depósitos e resgates, para o rendimento não contar dinheiro novo; se o saldo informado
                ficar muito longe do esperado, o app avisa, porque provavelmente faltou lançar um deles. Cofrinhos entram na carteira e na rentabilidade;
                FGTS, conta corrente e cartões benefício entram só no patrimônio. O saldo de 31/12 de cada um aparece em Impostos, para Bens e Direitos.
              </>
            }
          >
            Cofrinhos, FGTS, conta corrente e cartões benefício, pelo saldo que você vê no app do banco.
          </Intro>
        </div>
        {!form && (
          <button onClick={() => setForm({ ...EMPTY_NEW })} type="button">
            + Novo saldo
          </button>
        )}
      </div>

      {form && (
        <form className="card op-form" onSubmit={saveSavings}>
          <h3>{form.id ? "Alterar saldo" : "1. O que você quer acompanhar?"}</h3>
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
                  <span className="muted small">{k.example}</span>
                </button>
              ))}
            </div>
          )}
          {form.kind && (
            <>
              {!form.id && <h3 className="spaced">2. Dados</h3>}
              <div className="grid">
                <label>
                  {form.kind === "BENEFICIO" ? "Empresa do cartão" : "Banco"}
                  <input list="saldos-bancos" value={form.account} onChange={(e) => setForm({ ...form, account: e.target.value })} required />
                  <datalist id="saldos-bancos">
                    {accountNames.map((a) => (
                      <option key={a} value={a} />
                    ))}
                  </datalist>
                </label>
                <label>
                  Nome
                  <input value={form.name} placeholder={NAME_HINT[form.kind]} onChange={(e) => setForm({ ...form, name: e.target.value })} />
                </label>
              </div>
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

      {view && items.length > 0 && (
        <div className="savings-totals">
          <div>
            <span className="muted small">Cofrinhos (investimento)</span>
            <strong>{money(view.total_investments)}</strong>
          </div>
          <div>
            <span className="muted small">Outros saldos (FGTS, conta, benefício)</span>
            <strong>{money(view.total_others)}</strong>
          </div>
        </div>
      )}

      {!view ? (
        <p className="muted">Carregando…</p>
      ) : items.length === 0 ? (
        !form && <p className="muted">Nenhum saldo cadastrado ainda. Clique em "Novo saldo" para começar.</p>
      ) : (
        <div className="savings-list">
          {items.map((it) => {
            const s = it.savings;
            const lastCheck = it.checks[it.checks.length - 1];
            const isCofrinho = s.kind === "COFRINHO";
            return (
              <div className="card savings-card" key={s.id}>
                <div className="savings-head">
                  <div>
                    <span className="tag">{it.kind_label}</span>
                    <h3>
                      {s.name} <span className="muted">· {s.account}</span>
                    </h3>
                    <p className="muted small">
                      {it.last_informed ? `Último saldo informado em ${date(it.last_informed)}` : "Nenhum saldo informado ainda"}
                                          </p>
                  </div>
                  <div className="savings-balance">
                    <span className="muted small">{s.kind === "FGTS" ? "Saldo estimado hoje" : "Saldo"}</span>
                    <strong>{money(it.balance)}</strong>
                    {isCofrinho && Number(it.earned) !== 0 && (
                      <span className={`small ${tone(it.earned)}`}>
                        rendimento {money(it.earned)}
                      </span>
                    )}
                  </div>
                </div>

                {lastCheck?.suspicious && (
                  <p className="banner warning small">
                    O saldo de {date(lastCheck.date)} ficou {money(String(Math.abs(Number(lastCheck.difference))))}{" "}
                    {Number(lastCheck.difference) > 0 ? "acima" : "abaixo"} do esperado. Faltou lançar algum{" "}
                    {Number(lastCheck.difference) > 0 ? "depósito" : "resgate"}? Se não, pode ignorar: o app já usa o saldo informado.
                  </p>
                )}

                {!move || move.savingsId !== s.id ? (
                  <div className="actions">
                    <button type="button" onClick={() => startMove(it, "SALDO")}>
                      Atualizar saldo
                    </button>
                    {isCofrinho && (
                      <>
                        <button type="button" className="secondary" onClick={() => startMove(it, "DEPOSITO")}>
                          Depósito
                        </button>
                        <button type="button" className="secondary" onClick={() => startMove(it, "RESGATE")}>
                          Resgate
                        </button>
                      </>
                    )}
                    <span className="spacer" />
                    {it.moves.length > 0 && (
                      <button type="button" className="link" onClick={() => setOpen((o) => ({ ...o, [s.id]: !o[s.id] }))}>
                        {open[s.id] ? "Esconder movimentos" : `Ver movimentos (${it.moves.length})`}
                      </button>
                    )}
                    <button type="button" className="link" onClick={() => editSavings(s)}>
                      Alterar
                    </button>
                    <button type="button" className="link subtle" onClick={() => removeSavings(it)}>
                      Remover
                    </button>
                  </div>
                ) : (
                  moveForm(it)
                )}

                {open[s.id] && it.moves.length > 0 && (
                  <div className="scroll">
                    <table>
                      <thead>
                        <tr>
                          <th>Data</th>
                          <th>Movimento</th>
                          <th className="num">Valor</th>
                          <th>Detalhe</th>
                          <th />
                        </tr>
                      </thead>
                      <tbody>
                        {it.moves.map((m) => (
                          <tr key={m.id}>
                            <td>{date(m.date)}</td>
                            <td>{MOVE_LABEL[m.kind]}</td>
                            <td className="num">{money(m.amount)}</td>
                            <td className="muted small">{detail(it, m)}</td>
                            <td className="row-actions">
                              <button type="button" className="link" onClick={() => editMove(m)}>
                                Alterar
                              </button>
                              <button type="button" className="link subtle" onClick={() => removeMove(m)}>
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
