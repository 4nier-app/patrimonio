import { useEffect, useMemo, useState } from "react";
import {
  exitCostApi,
  errorMessage,
  taxApi,
  type DarfStatus,
  type ForeignYear,
  type GroupMonth,
  type SaleLine,
  type TaxGroup,
  type TaxMonth,
  type TaxView,
  type YearEndPosition,
} from "../api";
import { cashMask, date, money, parseDecimalInput, percent, quantity, toCashMask, tone } from "../format";

const MONTH_NAMES = ["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"];

const GROUP_LABEL: Record<TaxGroup, string> = {
  COMUM: "Operações comuns (15%)",
  DAY_TRADE: "Day trade (20%)",
  FII: "Fundos imobiliários e Fiagros (20%)",
};

const STATUS: Record<DarfStatus, { label: string; tone: string }> = {
  PAGO: { label: "Pago", tone: "ok" },
  PENDENTE: { label: "A pagar", tone: "warn" },
  ATRASADO: { label: "Atrasado", tone: "bad" },
  EM_ANDAMENTO: { label: "Mês em andamento", tone: "" },
  SEM_DARF: { label: "Sem DARF", tone: "" },
};

function monthName(key: string): string {
  const [y, m] = key.split("-");
  const name = MONTH_NAMES[Number(m) - 1] ?? m;
  return `${name[0].toUpperCase()}${name.slice(1)} de ${y}`;
}

const isZero = (v: string) => Number(v) === 0;

/** Pendências que aparecem no aviso: DARFs a pagar ou atrasados. */
export function pendingDarfs(view: TaxView | null): TaxMonth[] {
  return view?.months.filter((m) => m.status === "PENDENTE" || m.status === "ATRASADO") ?? [];
}

function PayForm({ month, onDone }: { month: TaxMonth; onDone: () => void }) {
  const today = new Date().toISOString().slice(0, 10);
  const [paidOn, setPaidOn] = useState(today);
  const [amount, setAmount] = useState(toCashMask(month.darf, 2).text);
  const [error, setError] = useState<string | null>(null);

  async function save() {
    const value = parseDecimalInput(amount);
    if (value == null) {
      setError("informe o valor pago");
      return;
    }
    try {
      await taxApi.markPaid({ month: month.month, paid_on: paidOn, amount: value });
      onDone();
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <div className="tax-pay">
      <label>
        Data do pagamento
        <input type="date" value={paidOn} onChange={(e) => setPaidOn(e.target.value)} />
      </label>
      <label>
        Valor pago
        <input inputMode="numeric" value={amount} onChange={(e) => setAmount(cashMask(e.target.value, 2))} />
      </label>
      <button onClick={save}>Confirmar pagamento</button>
      {error && <p className="error small">{error}</p>}
    </div>
  );
}

function GroupRows({ g }: { g: GroupMonth }) {
  return (
    <div className="tax-group">
      <h4>{GROUP_LABEL[g.group]}</h4>
      <dl>
        <dt>Resultado do mês</dt>
        <dd className={tone(g.result)}>{money(g.result)}</dd>
        {!isZero(g.loss_before) && (
          <>
            <dt>Prejuízo de meses anteriores</dt>
            <dd>{money(g.loss_before)}</dd>
            <dt>Prejuízo usado agora</dt>
            <dd>{money(g.loss_used)}</dd>
          </>
        )}
        <dt>Base de cálculo</dt>
        <dd>{money(g.base)}</dd>
        <dt>Imposto ({percent(g.rate)})</dt>
        <dd>
          <strong>{money(g.tax)}</strong>
        </dd>
        {!isZero(g.loss_after) && (
          <>
            <dt>Prejuízo que fica para os próximos meses</dt>
            <dd>{money(g.loss_after)}</dd>
          </>
        )}
      </dl>
    </div>
  );
}

function saleTag(s: SaleLine, exempt: boolean): string {
  if (s.day_trade) return s.group === "FII" ? "FII · day trade" : "day trade";
  if (s.group === "FII") return "FII";
  if (s.stock) return exempt ? "ação · isenta" : "ação";
  return "comum";
}

function Sales({ sales, exempt }: { sales: SaleLine[]; exempt: boolean }) {
  return (
    <div className="tax-sales">
      <h4>Vendas do mês</h4>
      {sales.map((s, i) => (
        <div key={i} className="tax-sale">
          <div className="tax-sale-head">
            <strong>{s.asset}</strong>
            <span className="ret-badge">{saleTag(s, exempt)}</span>
            <span className="muted small">
              {date(s.date)} · {s.account}
            </span>
          </div>
          <div className="tax-sale-calc small">
            {quantity(s.quantity)} × venda: {money(s.net)} líquido (bruto {money(s.gross)}) − custo {money(s.cost)} (preço médio{" "}
            {money(s.average_price)}) = <strong className={tone(s.result)}>{money(s.result)}</strong>
          </div>
        </div>
      ))}
    </div>
  );
}

function MonthCard({ m, code, onChanged }: { m: TaxMonth; code: string; onChanged: () => void }) {
  const [open, setOpen] = useState(m.status === "PENDENTE" || m.status === "ATRASADO");
  const [paying, setPaying] = useState(false);
  const st = STATUS[m.status];

  async function undo() {
    await taxApi.unmarkPaid(m.month);
    onChanged();
  }

  return (
    <div className={`card tax-month ${st.tone}`}>
      <div className="tax-month-head">
        <button className="link tax-month-title" onClick={() => setOpen(!open)}>
          <span className="ret-caret">{open ? "▾" : "▸"}</span>
          {monthName(m.month)}
        </button>
        <span className={`tax-status ${st.tone}`}>{st.label}</span>
        <div className="tax-month-value">
          {!isZero(m.darf) ? (
            <>
              <strong>{money(m.darf)}</strong>
              <span className="muted small">vence {date(m.due_date)}</span>
            </>
          ) : !isZero(m.small_after) ? (
            <span className="muted small">{money(m.small_after)} somam ao mês seguinte (DARF mínimo R$ 10,00)</span>
          ) : (
            <span className="muted small">nada a pagar</span>
          )}
        </div>
      </div>

      {m.paid && (
        <p className="small tax-paid">
          Pago em {date(m.paid.paid_on)}: {money(m.paid.amount)}.{" "}
          <button className="link" onClick={undo}>
            Desfazer
          </button>
        </p>
      )}
      {m.status === "ATRASADO" && (
        <p className="small error">
          O vencimento passou. Para pagar agora, gere o DARF com multa e juros no Sicalc da Receita Federal.
        </p>
      )}
      {!m.paid && !isZero(m.darf) && m.status !== "EM_ANDAMENTO" && (
        <>
          {paying ? (
            <PayForm
              month={m}
              onDone={() => {
                setPaying(false);
                onChanged();
              }}
            />
          ) : (
            <button className="secondary small-btn" onClick={() => setPaying(true)}>
              Marcar como pago
            </button>
          )}
        </>
      )}

      {open && (
        <div className="tax-detail">
          {!isZero(m.darf) && (
            <p className="small">
              DARF código <strong>{code}</strong>, período de apuração {date(`${m.month}-01`).slice(3)}, valor{" "}
              <strong>{money(m.darf)}</strong>, vencimento <strong>{date(m.due_date)}</strong>.
            </p>
          )}
          <p className="small muted">
            Vendas de ações no mês: {money(m.stock_sales)}.{" "}
            {m.exempt
              ? `Até R$ 20.000,00: o lucro com ações fica isento${isZero(m.exempt_profit) ? "" : ` (${money(m.exempt_profit)})`}.`
              : "Passou de R$ 20.000,00: o lucro com ações é tributado."}
          </p>
          {m.groups.map((g) => (
            <GroupRows key={g.group} g={g} />
          ))}
          <dl className="tax-total">
            <dt>Imposto dos grupos</dt>
            <dd>{money(m.gross_tax)}</dd>
            {(!isZero(m.irrf_month) || !isZero(m.irrf_before)) && (
              <>
                <dt>IRRF retido (estimado: 0,005% das vendas comuns e 1% do lucro de day trade)</dt>
                <dd>{money(m.irrf_month)}</dd>
                {!isZero(m.irrf_before) && (
                  <>
                    <dt>IRRF de meses anteriores</dt>
                    <dd>{money(m.irrf_before)}</dd>
                  </>
                )}
                <dt>IRRF abatido</dt>
                <dd>− {money(m.irrf_used)}</dd>
              </>
            )}
            {!isZero(m.small_before) && (
              <>
                <dt>Imposto de meses anteriores abaixo de R$ 10,00</dt>
                <dd>+ {money(m.small_before)}</dd>
              </>
            )}
            <dt>
              <strong>Total do mês</strong>
            </dt>
            <dd>
              <strong>{money(m.due)}</strong>
            </dd>
          </dl>
          {m.sales.length > 0 && <Sales sales={m.sales} exempt={m.exempt} />}
        </div>
      )}
    </div>
  );
}

function Foreign({ y }: { y: ForeignYear }) {
  return (
    <div className="card tax-month">
      <dl className="tax-total first">
        <dt>Resultado das vendas no ano</dt>
        <dd className={tone(y.result)}>{money(y.result)}</dd>
        {!isZero(y.loss_before) && (
          <>
            <dt>Prejuízo de anos anteriores</dt>
            <dd>{money(y.loss_before)}</dd>
            <dt>Prejuízo usado</dt>
            <dd>{money(y.loss_used)}</dd>
          </>
        )}
        <dt>Base de cálculo</dt>
        <dd>{money(y.base)}</dd>
        <dt>
          <strong>Imposto estimado ({percent(y.rate)}), pago com a declaração anual</strong>
        </dt>
        <dd>
          <strong>{money(y.tax)}</strong>
        </dd>
        {!isZero(y.loss_after) && (
          <>
            <dt>Prejuízo para os próximos anos</dt>
            <dd>{money(y.loss_after)}</dd>
          </>
        )}
      </dl>
      <div className="tax-sales">
        <h4>Vendas</h4>
        {y.sales.map((s, i) => (
          <div key={i} className="tax-sale">
            <div className="tax-sale-head">
              <strong>{s.asset}</strong>
              {s.crypto && <span className="ret-badge">cripto</span>}
              <span className="muted small">
                {date(s.date)} · {s.account}
              </span>
            </div>
            <div className="tax-sale-calc small">
              {quantity(s.quantity)} × venda: {money(s.net_usd, "USD")} × PTAX {s.ptax.replace(".", ",")} = {money(s.net_brl)} − custo{" "}
              {money(s.cost_brl)} = <strong className={tone(s.result_brl)}>{money(s.result_brl)}</strong>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

function Positions({ list, names, year }: { list: YearEndPosition[]; names: Record<string, string>; year: number }) {
  return (
    <div className="card">
      <p className="small muted">
        Posição em 31/12/{year} pelo custo de aquisição (preço médio com taxas), como pede a ficha Bens e Direitos. No
        exterior, o custo em reais soma cada compra pela PTAX da data. Fundos de investimento entram pelo valor aplicado
        (resgates e come-cotas tiram cotas pelo custo médio); confira com o informe de rendimentos do banco.
      </p>
      <table className="tax-positions">
        <thead>
          <tr>
            <th>Ativo</th>
            <th className="num">Quantidade</th>
            <th className="num">Custo</th>
          </tr>
        </thead>
        <tbody>
          {list.map((p) => (
            <tr key={p.asset}>
              <td>
                <strong>{p.asset}</strong>
                {names[p.asset] && <div className="muted small">{names[p.asset]}</div>}
              </td>
              <td className="num">{quantity(p.quantity)}</td>
              <td className="num">
                {money(p.cost_brl)}
                {p.cost_usd != null && <div className="muted small">{money(p.cost_usd, "USD")}</div>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

export default function Taxes({ refresh, onChanged }: { refresh: number; onChanged: () => void }) {
  const [view, setView] = useState<TaxView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [year, setYear] = useState<number | null>(null);
  const [resgate, setResgate] = useState(false);
  useEffect(() => {
    exitCostApi
      .table()
      .then((t) => setResgate(Object.keys(t).length > 0))
      .catch(() => {});
  }, [refresh]);

  function load() {
    taxApi
      .load()
      .then(setView)
      .catch((e) => setError(errorMessage(e)));
  }

  useEffect(load, [refresh]);

  const years = useMemo(() => {
    if (!view) return [];
    const set = new Set<number>();
    view.months.forEach((m) => set.add(Number(m.month.slice(0, 4))));
    view.annual.foreign.forEach((y) => set.add(y.year));
    Object.keys(view.annual.positions).forEach((y) => set.add(Number(y)));
    view.savings_year_end.forEach((x) => set.add(x.year));
    return [...set].sort((a, b) => b - a);
  }, [view]);

  useEffect(() => {
    if (year == null && years.length) setYear(years[0]);
  }, [years, year]);

  const pending = pendingDarfs(view);
  const months = view?.months.filter((m) => year != null && m.month.startsWith(String(year))) ?? [];
  const foreign = view?.annual.foreign.find((y) => y.year === year);
  const positions = (year != null && view?.annual.positions[String(year)]) || [];
  // Cartão benefício não vai para a declaração.
  const balances = view?.savings_year_end.filter((x) => x.year === year && x.kind !== "Cartão benefício") ?? [];

  function changed() {
    load();
    onChanged();
  }

  return (
    <section className="taxes-page">
      <div className="section-head">
        <div>
          <h2>Impostos</h2>
          <p className="muted">
            Imposto de renda sobre as vendas, calculado a partir dos lançamentos. B3: apuração mensal com DARF. Exterior e
            criptomoedas: apuração anual, na declaração. Vendas de FI-Infra (isentas para pessoa física) e de ETFs de renda fixa (o
            imposto é retido pelo próprio fundo) não entram no DARF; a posição continua em Bens e Direitos.
          </p>
        </div>
        {years.length > 0 && (
          <label className="ret-pick">
            Ano
            <select value={String(year ?? "")} onChange={(e) => setYear(Number(e.target.value))}>
              {years.map((y) => (
                <option key={y} value={y}>
                  {y}
                </option>
              ))}
            </select>
          </label>
        )}
      </div>

      <p className="warning tax-disclaimer" role="note">
        <b>Estes valores são uma estimativa.</b> Eles são calculados a partir dos lançamentos que você digitou, seguindo a legislação
        como o autor do aplicativo a entende. Podem ter erros ou não considerar a sua situação. Não são orientação fiscal nem
        contábil. Antes de pagar um DARF ou entregar a declaração, confira com um contador. O uso das informações é de sua
        responsabilidade.
      </p>

      {error && <p className="error">{error}</p>}

      {pending.length > 0 && (
        <div className="banner warning tax-alert" role="status">
          {pending.map((m) => (
            <div key={m.month}>
              DARF de {monthName(m.month).toLowerCase()}: <strong>{money(m.darf)}</strong>{" "}
              {m.status === "ATRASADO" ? `venceu em ${date(m.due_date)}` : `vence em ${date(m.due_date)}`}.
            </div>
          ))}
        </div>
      )}

      {view && view.warnings.length > 0 && (
        <ul className="tax-warnings small">
          {view.warnings.map((w) => (
            <li key={w}>{w}</li>
          ))}
        </ul>
      )}

      {view && years.length === 0 && <p className="muted">Nenhuma venda ainda. Os impostos aparecem aqui quando houver vendas.</p>}

      {year != null && (
        <>
          <h3 className="spaced">B3: apuração mensal</h3>
          {months.length === 0 ? (
            <p className="muted">Nenhuma venda na B3 em {year}.</p>
          ) : (
            months.map((m) => <MonthCard key={m.month} m={m} code={view!.darf_code} onChanged={changed} />)
          )}

          <h3 className="spaced">Exterior e criptomoedas: apuração anual</h3>
          {resgate && (
            <p className="banner info">
              ⓘ <strong>Aqui os valores em dólar usam a PTAX</strong> de venda de cada data, como exige a Lei 14.754/2023, sem descontar as taxas da
              corretora. Por isso podem ser um pouco maiores que o valor de resgate mostrado no Painel, na Evolução e na Carteira.
            </p>
          )}
          {foreign ? (
            <Foreign y={foreign} />
          ) : (
            <p className="muted">Nenhuma venda no exterior ou de criptomoedas em {year}.</p>
          )}
          <p className="small muted">
            Lei 14.754/2023: 15% sobre o ganho do ano em reais, cada operação convertida pela PTAX de venda da sua data.
            Criptomoedas em carteira própria podem ter outro tratamento; confirme com um contador.
          </p>

          {positions.length > 0 && (
            <>
              <h3 className="spaced">Bens e Direitos em 31/12/{year}</h3>
              <Positions list={positions} names={view!.names} year={year} />
            </>
          )}
          {balances.length > 0 && (
            <>
              <h3 className="spaced">Saldos e bens em 31/12/{year} (Bens e Direitos)</h3>
              <div className="card">
                <p className="small muted">
                  Cofrinhos e contas são declarados pelo saldo em 31/12 (cofrinho em "Aplicações de renda fixa", conta corrente em "Depósito em conta
                  corrente", se acima de R$ 140). O saldo é o último informado até 31/12 (informe o de dezembro na Evolução); confira com o informe de rendimentos
                  do banco. Imóveis e veículos entram pelo custo: valor pago mais as obras feitas até 31/12, sem valor de mercado.
                </p>
                <table className="tax-positions">
                  <thead>
                    <tr>
                      <th>Saldo ou bem</th>
                      <th>Tipo</th>
                      <th className="num">Em 31/12/{year}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {balances.map((b) => (
                      <tr key={`${b.kind}|${b.name}|${b.account}`}>
                        <td>
                          <strong>{b.name}</strong>
                          <div className="muted small">{b.account}</div>
                        </td>
                        <td>{b.kind}</td>
                        <td className="num">
                          {money(b.value)}
                          {b.usd != null && <div className="muted small">{money(b.usd, "USD")} × PTAX de 31/12</div>}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </>
          )}
        </>
      )}
    </section>
  );
}
