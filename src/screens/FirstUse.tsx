import { useEffect, useState } from "react";
import { api, errorMessage, market, savingsApi, type Account, type Currency, type SavingsKind } from "../api";
import CashInput from "../components/CashInput";
import { currencyOfNewAccount, fundTicker, normalizeTicker } from "../currency";
import { keepNumberChars, maskDecimal, money, parseDecimalInput, priceDecimals } from "../format";
import { setSimpleView } from "../viewMode";

/** Corretoras sugeridas no campo (dá para digitar outra). */
const BROKERS = ["Itaú", "XP", "BTG Pactual", "Nubank", "Rico", "Clear", "Inter", "Santander", "Bradesco", "Banco do Brasil", "Nomad", "Avenue", "Carteira física"];

type Row = { account: string; asset: string; quantity: string; price: string; done: boolean };
type Balance = { kind: SavingsKind; account: string; amount: string; done: boolean };

const emptyRow = (account = ""): Row => ({ account, asset: "", quantity: "", price: "", done: false });
const emptyBalance = (): Balance => ({ kind: "CONTA", account: "", amount: "", done: false });

const BALANCE_KINDS: { id: SavingsKind; label: string }[] = [
  { id: "CONTA", label: "Conta corrente" },
  { id: "FGTS", label: "FGTS" },
  { id: "COFRINHO", label: "Cofrinho / caixinha" },
];

function lastYearEnd(): string {
  return `${new Date().getFullYear() - 1}-12-31`;
}

function todayIso(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/**
 * Primeiro uso guiado: num perfil vazio, lança a carteira de hoje de uma vez
 * (cada ativo como uma compra só, pelo preço médio) e os saldos das contas.
 * O histórico pode ser completado depois.
 */
export default function FirstUse({ onDone }: { onDone: () => void }) {
  const [step, setStep] = useState<1 | 2 | 3>(1);
  const [simple, setSimple] = useState(true);
  const [date, setDate] = useState(lastYearEnd);
  const [rows, setRows] = useState<Row[]>([emptyRow(), emptyRow(), emptyRow()]);
  const [balances, setBalances] = useState<Balance[]>([emptyBalance()]);
  const [accounts, setAccounts] = useState<Account[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    market.listAccounts().then(setAccounts).catch(() => {});
  }, []);

  const fund = (r: Row) => fundTicker(r.asset);
  function currencyOf(r: Row): Currency {
    if (fund(r)) return "BRL";
    const known = accounts.find((a) => a.name.toLowerCase() === r.account.trim().toLowerCase());
    if (known) return known.currency;
    return currencyOfNewAccount(r.account, r.asset, []);
  }

  function setRow(i: number, patch: Partial<Row>) {
    setRows((list) =>
      list.map((r, k) => {
        if (k !== i) return r;
        const next = { ...r, ...patch };
        // Mudou a moeda (ex.: corretora no exterior): o preço tem outras casas
        // decimais e é digitado de novo.
        if (currencyOf(r) !== currencyOf(next)) next.price = "";
        return next;
      }),
    );
  }

  const filled = rows.filter((r) => r.asset.trim() || r.quantity.trim() || r.price.trim());
  const totalOf = (r: Row) => {
    const q = parseDecimalInput(r.quantity, currencyOf(r) === "USD");
    const p = parseDecimalInput(r.price);
    return q && p ? String(Number(q) * Number(p)) : null;
  };

  async function saveInvestments() {
    setError(null);
    for (const [i, r] of filled.entries()) {
      if (!r.account.trim()) return setError(`Linha ${i + 1}: informe a corretora.`);
      if (!r.asset.trim()) return setError(`Linha ${i + 1}: informe o ativo.`);
      if (!parseDecimalInput(r.quantity, currencyOf(r) === "USD")) return setError(`Linha ${i + 1}: quantidade inválida.`);
      if (!parseDecimalInput(r.price)) return setError(`Linha ${i + 1}: preço médio inválido.`);
    }
    if (!date) return setError("Informe a data da posição.");
    setBusy(true);
    try {
      for (let i = 0; i < rows.length; i++) {
        const r = rows[i];
        if (r.done || !(r.asset.trim() || r.quantity.trim() || r.price.trim())) continue;
        const currency = currencyOf(r);
        await market.create({
          id: "",
          account: r.account.trim(),
          asset: fund(r) ?? normalizeTicker(r.asset),
          date,
          side: "COMPRA",
          quantity: parseDecimalInput(r.quantity, currency === "USD")!,
          price: parseDecimalInput(r.price)!,
          // O preço médio da corretora já inclui as taxas.
          manual_fees: "0",
          note: "Posição inicial",
          currency,
          from_account: null,
          residue: null,
        });
        // Se a próxima falhar, esta não é lançada de novo.
        setRows((list) => list.map((x, k) => (k === i ? { ...x, done: true } : x)));
      }
      setStep(2);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  const filledBalances = balances.filter((b) => b.amount.trim() || b.account.trim());

  async function saveBalances() {
    setError(null);
    for (const [i, b] of filledBalances.entries()) {
      if (b.kind !== "FGTS" && !b.account.trim()) return setError(`Linha ${i + 1}: informe o banco.`);
      if (parseDecimalInput(b.amount) == null) return setError(`Linha ${i + 1}: informe o saldo.`);
    }
    const today = todayIso();
    setBusy(true);
    try {
      for (let i = 0; i < balances.length; i++) {
        const b = balances[i];
        if (b.done || !(b.amount.trim() || b.account.trim())) continue;
        const item = await savingsApi.create({
          id: "",
          kind: b.kind,
          account: b.kind === "FGTS" ? "" : b.account.trim(),
          name: "",
          rate_pct: "100",
          tax_free: false,
          note: null,
          since: `${today.slice(0, 7)}-01`,
          until: null,
          fixed: null,
        });
        await savingsApi.createMove({
          id: "",
          savings_id: item.id,
          date: today,
          kind: "SALDO",
          amount: parseDecimalInput(b.amount)!,
          ir: null,
          iof: null,
          note: null,
        });
        setBalances((list) => list.map((x, k) => (k === i ? { ...x, done: true } : x)));
      }
      setStep(3);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  }

  async function finish() {
    setBusy(true);
    try {
      await api.finishFirstUse();
      if (step === 3) setSimpleView(simple);
      onDone();
    } catch (e) {
      setError(errorMessage(e));
      setBusy(false);
    }
  }

  const brokers = Array.from(new Set([...accounts.map((a) => a.name), ...BROKERS]));
  const steps = ["Investimentos", "Contas e reservas", "Pronto"];

  return (
    <section className="first-use stack">
      <div className="card stack">
        <ol className="first-use-steps">
          {steps.map((s, i) => (
            <li key={s} className={i + 1 === step ? "active" : i + 1 < step ? "done" : ""}>
              {i + 1}. {s}
            </li>
          ))}
        </ol>

        {step === 1 && (
          <>
            <h2>Vamos montar a sua carteira de hoje</h2>
            <p className="muted">
              Informe o que você tem hoje em cada corretora, com a quantidade e o preço médio que aparecem no app da corretora. Não precisa do
              histórico: cada ativo entra como uma compra só, na data abaixo. Se depois quiser importar as notas antigas, é só apagar a posição
              inicial em Lançamentos.
            </p>
            <label className="first-use-date">
              Posição em
              <input type="date" value={date} onChange={(e) => setDate(e.target.value)} />
              <span className="muted small">Dica: 31/12 do ano passado bate com a sua última declaração do imposto de renda.</span>
            </label>
            <div className="scroll">
              <table className="first-use-table">
                <thead>
                  <tr>
                    <th>Corretora</th>
                    <th>Ativo</th>
                    <th className="num">Quantidade</th>
                    <th className="num">Preço médio</th>
                    <th className="num">Custo total</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {rows.map((r, i) => {
                    const cur = currencyOf(r);
                    const total = totalOf(r);
                    return (
                      <tr key={i}>
                        <td>
                          <input list="first-use-brokers" value={r.account} disabled={r.done} placeholder="Ex.: Itaú" onChange={(e) => setRow(i, { account: e.target.value })} />
                        </td>
                        <td>
                          <input value={r.asset} disabled={r.done} placeholder="Ex.: ITSA4" onChange={(e) => setRow(i, { asset: e.target.value })} />
                        </td>
                        <td className="num">
                          <input
                            className="num"
                            inputMode="decimal"
                            value={r.quantity}
                            disabled={r.done}
                            onChange={(e) => setRow(i, { quantity: keepNumberChars(e.target.value) })}
                            onBlur={() => setRow(i, { quantity: maskDecimal(r.quantity, "quantity", cur, cur === "USD") })}
                          />
                        </td>
                        <td className="num">
                          {r.done ? (
                            <input className="num" value={r.price} disabled />
                          ) : (
                            <CashInput value={r.price} decimals={priceDecimals(cur)} currency={cur} onChange={(v) => setRow(i, { price: v })} />
                          )}
                        </td>
                        <td className="num">{total ? money(total, cur) : "—"}</td>
                        <td>
                          {r.done ? (
                            <span className="ok small">lançado</span>
                          ) : (
                            <button type="button" className="link danger" onClick={() => setRows((l) => (l.length > 1 ? l.filter((_, k) => k !== i) : [emptyRow()]))}>
                              Remover
                            </button>
                          )}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <datalist id="first-use-brokers">
              {brokers.map((b) => (
                <option key={b} value={b} />
              ))}
            </datalist>
            <div>
              <button type="button" className="secondary" onClick={() => setRows((l) => [...l, emptyRow(l[l.length - 1]?.account ?? "")])}>
                + Adicionar ativo
              </button>
            </div>
            <p className="muted small">
              Ativos em dólar: o custo em reais usa a PTAX da data da posição (para o imposto do exterior ficar exato, o ideal é lançar cada
              compra). Fundos de investimento: digite o CNPJ do fundo, a quantidade de cotas e o valor da cota na data. Cripto vai na “Carteira
              física”.
            </p>
          </>
        )}

        {step === 2 && (
          <>
            <h2>Contas e reservas</h2>
            <p className="muted">
              Agora o dinheiro parado: conta corrente, FGTS e cofrinhos (caixinhas). Informe o saldo de hoje. Nos meses seguintes, você atualiza o
              saldo direto na Evolução, clicando na célula. Se não quiser agora, é só seguir.
            </p>
            <div className="scroll">
              <table className="first-use-table">
                <thead>
                  <tr>
                    <th>Tipo</th>
                    <th>Banco</th>
                    <th className="num">Saldo hoje</th>
                    <th />
                  </tr>
                </thead>
                <tbody>
                  {balances.map((b, i) => (
                    <tr key={i}>
                      <td>
                        <select
                          value={b.kind}
                          disabled={b.done}
                          onChange={(e) => setBalances((l) => l.map((x, k) => (k === i ? { ...x, kind: e.target.value as SavingsKind } : x)))}
                        >
                          {BALANCE_KINDS.map((k) => (
                            <option key={k.id} value={k.id}>
                              {k.label}
                            </option>
                          ))}
                        </select>
                      </td>
                      <td>
                        {b.kind === "FGTS" ? (
                          <span className="muted small">—</span>
                        ) : (
                          <input
                            list="first-use-brokers"
                            value={b.account}
                            disabled={b.done}
                            placeholder="Ex.: Itaú"
                            onChange={(e) => setBalances((l) => l.map((x, k) => (k === i ? { ...x, account: e.target.value } : x)))}
                          />
                        )}
                      </td>
                      <td className="num">
                        {b.done ? (
                          <input className="num" value={b.amount} disabled />
                        ) : (
                          <CashInput
                            value={b.amount}
                            decimals={2}
                            currency="BRL"
                            onChange={(v) => setBalances((l) => l.map((x, k) => (k === i ? { ...x, amount: v } : x)))}
                          />
                        )}
                      </td>
                      <td>
                        {b.done ? (
                          <span className="ok small">lançado</span>
                        ) : (
                          <button type="button" className="link danger" onClick={() => setBalances((l) => (l.length > 1 ? l.filter((_, k) => k !== i) : [emptyBalance()]))}>
                            Remover
                          </button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div>
              <button type="button" className="secondary" onClick={() => setBalances((l) => [...l, emptyBalance()])}>
                + Adicionar saldo
              </button>
            </div>
            <p className="muted small">Cofrinho entra rendendo 100% do CDI; ajuste a taxa depois em Evolução › Contas e reservas, se for outra.</p>
          </>
        )}

        {step === 3 && (
          <>
            <h2>Pronto!</h2>
            <p>
              {rows.filter((r) => r.done).length} ativo(s) e {balances.filter((b) => b.done).length} saldo(s) lançados. O aplicativo vai buscar as
              cotações agora e abrir o Painel.
            </p>
            <fieldset className="first-use-view">
              <legend>Como prefere ver?</legend>
              <label>
                <input type="radio" name="visao" checked={simple} onChange={() => setSimple(true)} />
                <span>
                  <b>Visão simples</b>: quanto você tem, quanto rendeu no ano e se ganhou do CDI, e uma carteira com poucas colunas.
                </span>
              </label>
              <label>
                <input type="radio" name="visao" checked={!simple} onChange={() => setSimple(false)} />
                <span>
                  <b>Visão completa</b>: todas as abas, com a evolução mês a mês, a rentabilidade detalhada e o home broker.
                </span>
              </label>
              <span className="muted small">Dá para trocar quando quiser, no seletor do topo.</span>
            </fieldset>
            <div className="muted">
              Próximos passos, quando quiser:
              <ul className="first-use-tips">
                <li>Traga o histórico da bolsa em Lançamentos › Importar da B3: o extrato de negociação traz as compras e vendas, e o de movimentação, os proventos, bonificações e desdobramentos.</li>
                <li>Importe as próximas notas de corretagem em Lançamentos › Importar notas.</li>
                <li>Veja em Lançamentos › Proventos os que faltam lançar.</li>
                <li>Defina a divisão ideal da carteira em Carteira › Alocação e uma meta de patrimônio no Painel.</li>
                <li>Faça um backup em Segurança e guarde fora deste computador.</li>
              </ul>
            </div>
          </>
        )}

        {error && <p className="error">{error}</p>}

        <div className="first-use-actions">
          {step < 3 ? (
            <button type="button" className="link" disabled={busy} onClick={() => void finish()}>
              {step === 1 ? "Pular, vou lançar depois" : "Terminar sem saldos"}
            </button>
          ) : (
            <span />
          )}
          {step === 1 && (
            <button type="button" disabled={busy} onClick={() => void saveInvestments()}>
              {busy ? "Lançando…" : filled.length ? "Lançar e seguir: contas e reservas →" : "Seguir: contas e reservas →"}
            </button>
          )}
          {step === 2 && (
            <button type="button" disabled={busy} onClick={() => void saveBalances()}>
              {busy ? "Lançando…" : "Seguir →"}
            </button>
          )}
          {step === 3 && (
            <button type="button" disabled={busy} onClick={() => void finish()}>
              Abrir o Painel
            </button>
          )}
        </div>
      </div>
    </section>
  );
}
