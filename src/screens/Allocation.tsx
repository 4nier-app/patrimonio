import { useEffect, useMemo, useState } from "react";
import Intro from "../components/Intro";
import CashInput from "../components/CashInput";
import { allocationApi, errorMessage, evolutionApi } from "../api";
import { CLASSES, NO_DEPOSIT, currentValues, lines, type ClassId } from "../allocation";
import { money, parseDecimalInput } from "../format";

const pct = (n: number | null) => (n == null ? "—" : `${n.toLocaleString("pt-BR", { minimumFractionDigits: 1, maximumFractionDigits: 1 })}%`);
const brl = (n: number) => money(String(n));

/** Texto do campo de meta: "25" ou "12,5". */
function parseTarget(raw: string): number | null {
  const d = parseDecimalInput(raw.replace("%", ""));
  return d == null ? null : Number(d);
}

/** Carteira › Alocação: meta de cada classe e onde colocar o próximo aporte. */
export default function Allocation({ refresh }: { refresh: number }) {
  const [values, setValues] = useState<Record<ClassId, number> | null>(null);
  const [missing, setMissing] = useState(false);
  const [draft, setDraft] = useState<Record<string, string>>({});
  const [saved, setSaved] = useState<Partial<Record<ClassId, number>>>({});
  const [amount, setAmount] = useState("");
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    evolutionApi
      .load(new Date().getFullYear())
      .then((v) => {
        const c = currentValues(v);
        setValues(c.values);
        setMissing(c.missing);
      })
      .catch((e) => setError(errorMessage(e)));
    allocationApi
      .targets()
      .then((t) => {
        const nums = Object.fromEntries(Object.entries(t).map(([k, v]) => [k, Number(v)])) as Partial<Record<ClassId, number>>;
        setSaved(nums);
        setDraft(Object.fromEntries(Object.entries(nums).map(([k, v]) => [k, String(v).replace(".", ",")])));
      })
      .catch(() => {});
  }, [refresh]);

  const targets = useMemo(() => {
    const t: Partial<Record<ClassId, number>> = {};
    for (const c of CLASSES) {
      const n = parseTarget(draft[c.id] ?? "");
      if (n != null) t[c.id] = n;
    }
    return t;
  }, [draft]);
  const sum = Object.values(targets).reduce((s, v) => s + (v ?? 0), 0);
  const sumOk = Math.abs(sum - 100) < 0.0001;
  const invalid = CLASSES.some((c) => (draft[c.id] ?? "").trim() !== "" && parseTarget(draft[c.id]) == null);
  const changed = JSON.stringify(targets) !== JSON.stringify(Object.fromEntries(CLASSES.filter((c) => saved[c.id] != null).map((c) => [c.id, saved[c.id]])));
  const deposit = Number(parseDecimalInput(amount) ?? "0");
  // Enquanto as metas digitadas não somam 100%, a conta usa as salvas.
  const active = sumOk ? targets : saved;
  const view = values ? lines(values, active, deposit) : [];
  const total = view.filter((l) => l.target != null).reduce((s, l) => s + l.value, 0);

  async function save() {
    setMessage(null);
    try {
      const payload = Object.fromEntries(Object.entries(targets).map(([k, v]) => [k, String(v)]));
      await allocationApi.save(payload);
      setSaved(targets);
      setMessage({ ok: true, text: Object.keys(targets).length ? "Metas salvas." : "Metas apagadas." });
    } catch (e) {
      setMessage({ ok: false, text: errorMessage(e) });
    }
  }

  if (error) return <p className="error">{error}</p>;
  if (!values) return <p className="muted">Carregando…</p>;

  return (
    <section className="allocation">
      <h2>Alocação</h2>
      <Intro
        more={
          <>
            Escolha quanto do patrimônio você quer em cada classe. Só entram na conta as classes com meta; deixe em branco o que não quer
            acompanhar (os bens nunca entram). As metas precisam somar 100%. O valor de hoje é o mesmo da coluna "hoje" da Evolução: o que está em
            dólar vai pela PTAX, ou pelo valor de resgate quando a corretora tem o custo para trazer ao Brasil informado. Em "Onde investir", o
            aporte vai só para as classes abaixo da meta, sem vender nada: primeiro para as que estão mais longe da meta, em proporção ao
            tamanho da meta de cada uma, até que todas fiquem igualmente perto dela. O FGTS
            conta no total, mas não recebe aporte. É uma sugestão: confira antes de investir.
          </>
        }
      >
        Defina a meta de cada classe e veja quanto falta em cada uma e onde colocar o próximo aporte.
      </Intro>

      {missing && <p className="banner warning small">Faltou cotação de algum ativo hoje; o valor dele não entrou na conta.</p>}

      <div className="card">
        <div className="scroll">
          <table className="alloc-table">
            <thead>
              <tr>
                <th>Classe</th>
                <th className="num">Hoje</th>
                <th className="num">% hoje</th>
                <th className="num">Meta</th>
                <th className="num">Para chegar na meta</th>
              </tr>
            </thead>
            <tbody>
              {view.map((l) => (
                <tr key={l.id} className={l.target == null ? "muted" : ""}>
                  <td>{l.label}</td>
                  <td className="num">{brl(l.value)}</td>
                  <td className="num">{pct(l.share)}</td>
                  <td className="num">
                    <input
                      className="alloc-target"
                      inputMode="decimal"
                      placeholder="—"
                      aria-label={`Meta de ${l.label} em %`}
                      value={draft[l.id] ?? ""}
                      onChange={(e) => setDraft((d) => ({ ...d, [l.id]: e.target.value.replace(/[^\d,]/g, "") }))}
                    />
                    %
                  </td>
                  <td className={`num ${l.gap == null ? "" : l.gap > 0.005 ? "pos" : l.gap < -0.005 ? "neg" : ""}`}>
                    {l.gap == null ? "" : Math.abs(l.gap) < 0.005 ? "na meta" : l.gap > 0 ? `faltam ${brl(l.gap)}` : `sobram ${brl(-l.gap)}`}
                  </td>
                </tr>
              ))}
            </tbody>
            <tfoot>
              <tr>
                <th>Total com meta</th>
                <th className="num">{brl(total)}</th>
                <th />
                <th className={`num ${sumOk || !Object.keys(targets).length ? "" : "neg"}`}>{pct(sum)}</th>
                <th />
              </tr>
            </tfoot>
          </table>
        </div>
        <div className="actions">
          <button onClick={() => void save()} disabled={!changed || invalid || (Object.keys(targets).length > 0 && !sumOk)}>
            Salvar metas
          </button>
          {Object.keys(targets).length > 0 && !sumOk && (
            <span className="warning-text small">
              As metas somam {pct(sum)}; {sum < 100 ? `faltam ${pct(100 - sum)}` : `passaram ${pct(sum - 100)}`}.
            </span>
          )}
          {message && <span className={message.ok ? "ok" : "error"}>{message.text}</span>}
        </div>
      </div>

      {Object.keys(active).length > 0 && (
        <div className="card stack">
          <h3>Onde investir</h3>
          <label className="alloc-amount">
            Quanto vai investir agora?
            <CashInput value={amount} decimals={2} currency="BRL" onChange={setAmount} />
          </label>
          {deposit > 0 && (
            <table className="alloc-table">
              <thead>
                <tr>
                  <th>Classe</th>
                  <th className="num">Investir</th>
                  <th className="num">% depois do aporte</th>
                </tr>
              </thead>
              <tbody>
                {view
                  .filter((l) => l.target != null)
                  .map((l) => (
                    <tr key={l.id} className={l.deposit > 0 ? "" : "muted"}>
                      <td>
                        {l.label}
                        {NO_DEPOSIT.includes(l.id) && <span className="muted small"> · não recebe aporte</span>}
                      </td>
                      <td className="num">{l.deposit > 0 ? brl(l.deposit) : "—"}</td>
                      <td className="num">
                        {pct(((l.value + l.deposit) / (total + deposit)) * 100)} <span className="muted small">(meta {pct(l.target)})</span>
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          )}
        </div>
      )}
    </section>
  );
}
