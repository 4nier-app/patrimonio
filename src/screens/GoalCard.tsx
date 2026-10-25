import { useEffect, useState } from "react";
import { errorMessage, evolutionApi, goalApi, type EvolutionView, type Goal } from "../api";
import CashInput from "../components/CashInput";
import { date, money, monthLabel, parseDecimalInput, toCashMask } from "../format";
import { progress, type Point } from "../goal";
import { withoutOwned } from "./Evolution";

/** Valor do patrimônio em cada coluna da Evolução, com ou sem os bens. */
function points(view: EvolutionView, withOwned: boolean): Point[] {
  const values = withOwned ? view.total : (withoutOwned(view)?.values ?? view.total);
  return view.columns.flatMap((c, i) => (values[i] == null ? [] : [{ date: c.date, value: Number(values[i]) }]));
}

const brl = (n: number) => money(n.toFixed(2));
const monthYear = (iso: string) => `${monthLabel(Number(iso.slice(5, 7)))}/${iso.slice(0, 4)}`;

/** Painel › Meta de patrimônio. */
export default function GoalCard({ year, refresh }: { year: EvolutionView; refresh: number }) {
  const [goal, setGoal] = useState<Goal | null | undefined>(undefined);
  const [previous, setPrevious] = useState<EvolutionView | null>(null);
  const [editing, setEditing] = useState(false);
  const [form, setForm] = useState({ value: "", date: "", withOwned: false });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    goalApi
      .load()
      .then(setGoal)
      .catch(() => setGoal(null));
  }, [refresh]);

  // Ano anterior, para o ritmo dos últimos 12 meses.
  useEffect(() => {
    const y = (year.year ?? new Date().getFullYear()) - 1;
    if (!year.years.includes(y)) return;
    evolutionApi
      .load(y)
      .then(setPrevious)
      .catch(() => {});
  }, [year, refresh]);

  if (goal === undefined) return null;

  function edit() {
    setError(null);
    setForm(
      goal
        ? { value: toCashMask(goal.value, 2).text, date: goal.date, withOwned: goal.with_owned }
        : { value: "", date: `${new Date().getFullYear() + 5}-12-31`, withOwned: false },
    );
    setEditing(true);
  }

  async function save(next: Goal | null) {
    setError(null);
    try {
      await goalApi.save(next);
      setGoal(next);
      setEditing(false);
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  function submit(e: React.FormEvent) {
    e.preventDefault();
    const value = parseDecimalInput(form.value);
    if (!value || Number(value) <= 0) return setError("Informe o valor da meta.");
    if (!form.date) return setError("Informe a data da meta.");
    void save({ value, date: form.date, with_owned: form.withOwned });
  }

  const hasOwned = year.groups.some((g) => g.id === "bens");

  if (editing) {
    return (
      <form className="card goal stack" onSubmit={submit}>
        <h3>Meta de patrimônio</h3>
        <div className="goal-form">
          <label>
            Quanto quer ter
            <CashInput value={form.value} decimals={2} currency="BRL" onChange={(v) => setForm((f) => ({ ...f, value: v }))} required />
          </label>
          <label>
            Até quando
            <input type="date" value={form.date} onChange={(e) => setForm((f) => ({ ...f, date: e.target.value }))} required />
          </label>
          {hasOwned && (
            <label className="check">
              <input type="checkbox" checked={form.withOwned} onChange={(e) => setForm((f) => ({ ...f, withOwned: e.target.checked }))} />
              Contar os bens (imóveis e veículos)
            </label>
          )}
        </div>
        {error && <p className="error">{error}</p>}
        <div className="actions wrap">
          <button type="submit">Salvar meta</button>
          <button type="button" className="secondary" onClick={() => setEditing(false)}>
            Cancelar
          </button>
          {goal && (
            <button type="button" className="secondary" onClick={() => void save(null)}>
              Apagar meta
            </button>
          )}
        </div>
      </form>
    );
  }

  if (!goal) {
    return (
      <div className="card goal goal-empty">
        <div>
          <h3>Meta de patrimônio</h3>
          <p className="muted small">Diga quanto quer ter e até quando. O Painel mostra quanto falta e se o seu ritmo leva você lá a tempo.</p>
        </div>
        <button type="button" className="secondary" onClick={edit}>
          Definir meta
        </button>
      </div>
    );
  }

  const current = points(year, goal.with_owned);
  const now = current[current.length - 1];
  if (!now) return null;
  const history = [...(previous ? points(previous, goal.with_owned) : []), ...current];
  const target = Number(goal.value);
  const p = progress(now, { value: target, date: goal.date }, history);
  const reached = p.missing === 0;
  const late = p.arrival != null && p.arrival > goal.date;
  const what = goal.with_owned ? "patrimônio" : "patrimônio sem os bens";

  return (
    <div className="card goal stack">
      <div className="block-head">
        <h3>Meta de patrimônio</h3>
        <button type="button" className="link" onClick={edit}>
          Alterar
        </button>
      </div>
      <div className="goal-numbers">
        <strong>{brl(now.value)}</strong>
        <span className="muted">
          de {brl(target)} até {date(goal.date)}
          {!goal.with_owned && hasOwned && " · sem os bens"}
        </span>
      </div>
      <div className="goal-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(Math.min(1, p.done) * 100)}>
        <span style={{ width: `${Math.min(1, Math.max(0, p.done)) * 100}%` }} />
      </div>
      <p className="small">
        {reached ? (
          <strong className="pos">Meta alcançada! Você já tem {Math.round(p.done * 100)}% do valor.</strong>
        ) : (
          <>
            {Math.round(p.done * 100)}% da meta. Faltam <strong>{brl(p.missing)}</strong>
            {p.perMonth != null
              ? `: o ${what} precisa crescer ${brl(p.perMonth)} por mês até ${date(goal.date)}, somando aportes e rendimentos.`
              : ", e a data da meta já passou."}
          </>
        )}
      </p>
      {!reached && p.pace != null && (
        <p className={`small ${late || p.pace <= 0 ? "warning-text" : "pos"}`}>
          {p.pace <= 0
            ? `Nos últimos ${Math.round(p.paceMonths ?? 0)} meses o ${what} não cresceu; nesse ritmo, a meta não chega.`
            : `Nos últimos ${Math.round(p.paceMonths ?? 0)} meses ele cresceu ${brl(p.pace)} por mês. Nesse ritmo, você chega lá em ${monthYear(p.arrival!)}, ${late ? "depois" : "antes"} do prazo.`}
        </p>
      )}
    </div>
  );
}
