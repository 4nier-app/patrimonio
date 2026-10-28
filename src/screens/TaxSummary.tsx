import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import {
  errorMessage,
  eventsApi,
  incomesApi,
  market,
  taxApi,
  type EventRow,
  type IncomeRow,
  type SuggestedIncome,
  type TaxGroup,
  type TaxView,
} from "../api";
import {
  bensCode,
  bonificacoes,
  discriminacao,
  incomeLines,
  pendencias,
  type IncomeLine,
} from "../declaracao";
import { money, tone } from "../format";

const MONTHS = [
  "Janeiro",
  "Fevereiro",
  "Março",
  "Abril",
  "Maio",
  "Junho",
  "Julho",
  "Agosto",
  "Setembro",
  "Outubro",
  "Novembro",
  "Dezembro",
];
const GROUPS: { id: TaxGroup; label: string }[] = [
  { id: "COMUM", label: "Comuns" },
  { id: "DAY_TRADE", label: "Day trade" },
  { id: "FII", label: "FIIs" },
];

const brl = (v: number) => money(v.toFixed(2));

/** O mesmo aviso da tela de Impostos; no papel, repete no topo de cada página. */
export const DISCLAIMER = (
  <>
    <b>Estes valores são uma estimativa.</b> Eles são calculados a partir dos
    lançamentos que você digitou, seguindo a legislação como o autor do
    aplicativo a entende. Podem ter erros ou não considerar a sua situação. Não
    são orientação fiscal nem contábil. Antes de pagar um DARF ou entregar a
    declaração, confira com um contador. O uso das informações é de sua
    responsabilidade.
  </>
);

type Data = {
  view: TaxView;
  incomes: IncomeRow[];
  events: EventRow[];
  kinds: Record<string, string>;
  names: Record<string, string>;
};

function Lines({ lines }: { lines: IncomeLine[] }) {
  return (
    <table className="resumo-table">
      <tbody>
        {lines.map((l) => (
          <tr key={l.id}>
            <td>
              {l.label}
              <div className="muted small">{l.assets.join(", ")}</div>
              {l.withheld > 0 && (
                <div className="muted small">
                  Bruto {brl(l.gross)}, com {brl(l.withheld)} de imposto já
                  retido na fonte.
                </div>
              )}
            </td>
            <td className="num">
              <strong>{brl(l.net)}</strong>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

function Body({
  data,
  year,
  print,
}: {
  data: Data;
  year: number;
  print?: boolean;
}) {
  const { view, incomes, events, kinds } = data;
  const names = { ...data.names, ...view.names };
  const positions = view.annual.positions[String(year)] ?? [];
  const balances = view.savings_year_end.filter(
    (x) => x.year === year && x.kind !== "Cartão benefício",
  );
  const inc = incomeLines(incomes, year);
  const bonus = bonificacoes(events, year);
  const months = view.months.filter(
    (m) => m.month.startsWith(String(year)) && m.sales.length > 0,
  );
  const ofYear = view.months.filter((m) => m.month.startsWith(String(year)));
  const lastMonth = ofYear[ofYear.length - 1];
  const foreign = view.annual.foreign.find((y) => y.year === year);
  const usedGroups = GROUPS.filter((g) =>
    months.some((m) => m.groups.some((x) => x.group === g.id)),
  );

  return (
    <div className="resumo-body">
      <section className="card resumo-ficha">
        <div className="resumo-head">
          <h3>Bens e Direitos</h3>
          <span className="muted small">
            situação em 31/12/{year}, pelo custo de aquisição
          </span>
        </div>
        {positions.length === 0 && balances.length === 0 ? (
          <p className="muted small">Nada na carteira em 31/12/{year}.</p>
        ) : (
          <table className="resumo-table">
            <tbody>
              {positions.map((p) => {
                const code = bensCode(p.asset, kinds[p.asset]);
                return (
                  <tr key={p.asset}>
                    <td>
                      {code ? (
                        <>
                          Grupo {code.group} · Código {code.code} ·{" "}
                          <b>{p.asset}</b>
                        </>
                      ) : (
                        <>
                          <b>{p.asset}</b>{" "}
                          <span className="muted small">
                            · grupo e código: confirme com o contador
                          </span>
                        </>
                      )}
                      <div className="muted small">
                        {discriminacao(p, kinds[p.asset], names[p.asset])}
                      </div>
                    </td>
                    <td className="num">
                      <strong>{money(p.cost_brl)}</strong>
                      {p.cost_usd != null && (
                        <div className="muted small">
                          {money(p.cost_usd, "USD")}
                        </div>
                      )}
                    </td>
                  </tr>
                );
              })}
              {balances.map((b) => (
                <tr key={`${b.kind}|${b.name}|${b.account}`}>
                  <td>
                    {b.kind === "Conta corrente"
                      ? "Grupo 06 · Código 01 · "
                      : ""}
                    <b>{b.name}</b>
                    <div className="muted small">
                      {b.kind}
                      {b.account ? ` · ${b.account}` : ""}
                    </div>
                  </td>
                  <td className="num">
                    <strong>{money(b.value)}</strong>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <p className="muted small">
          No exterior, o custo em reais soma cada compra pela PTAX da data.
          Confira com o informe de rendimentos de cada corretora e banco.
        </p>
      </section>

      <section className="card resumo-ficha">
        <div className="resumo-head">
          <h3>Rendimentos isentos e não tributáveis</h3>
        </div>
        {inc.isentos.length === 0 && bonus.total === 0 ? (
          <p className="muted small">
            Nenhum provento isento lançado em {year}.
          </p>
        ) : (
          <>
            <Lines lines={inc.isentos} />
            {bonus.total > 0 && (
              <Lines
                lines={[
                  {
                    id: "bonus",
                    label: "Bonificações em ações",
                    net: bonus.total,
                    gross: bonus.total,
                    withheld: 0,
                    assets: bonus.assets,
                  },
                ]}
              />
            )}
          </>
        )}
      </section>

      <section className="card resumo-ficha">
        <div className="resumo-head">
          <h3>Rendimentos sujeitos à tributação exclusiva</h3>
        </div>
        {inc.exclusivos.length === 0 ? (
          <p className="muted small">Nenhum JCP ou juros lançado em {year}.</p>
        ) : (
          <Lines lines={inc.exclusivos} />
        )}
      </section>

      <section className="card resumo-ficha">
        <div className="resumo-head">
          <h3>Renda variável</h3>
          <span className="muted small">
            resultado de cada mês na B3, já com o prejuízo compensado
          </span>
        </div>
        {months.length === 0 ? (
          <p className="muted small">Nenhuma venda na B3 em {year}.</p>
        ) : (
          <table className="resumo-table resumo-months">
            <thead>
              <tr>
                <th>Mês</th>
                {usedGroups.map((g) => (
                  <th key={g.id} className="num">
                    {g.label}
                  </th>
                ))}
                <th className="num">DARF</th>
              </tr>
            </thead>
            <tbody>
              {months.map((m) => (
                <tr key={m.month}>
                  <td>{MONTHS[Number(m.month.slice(5, 7)) - 1]}</td>
                  {usedGroups.map((g) => {
                    const x = m.groups.find((y) => y.group === g.id);
                    return (
                      <td
                        key={g.id}
                        className={`num ${x ? tone(x.result) : ""}`}
                      >
                        {x ? money(x.result) : "—"}
                      </td>
                    );
                  })}
                  <td className="num">
                    {Number(m.darf) > 0 ? money(m.darf) : "—"}
                    {m.paid && <div className="muted small">pago</div>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        {lastMonth &&
          lastMonth.groups.some((g) => Number(g.loss_after) > 0) && (
            <p className="small">
              Prejuízo a compensar no fim do ano:{" "}
              {lastMonth.groups
                .filter((g) => Number(g.loss_after) > 0)
                .map(
                  (g) =>
                    `${GROUPS.find((x) => x.id === g.group)?.label ?? g.group} ${money(g.loss_after)}`,
                )
                .join(" · ")}
              .
            </p>
          )}
      </section>

      <section className="card resumo-ficha">
        <div className="resumo-head">
          <h3>Aplicações financeiras no exterior (Lei 14.754/2023)</h3>
          <span className="muted small">pela PTAX de cada data</span>
        </div>
        {!foreign && !inc.exterior ? (
          <p className="muted small">
            Nenhuma venda nem provento no exterior ou em criptomoedas em {year}.
          </p>
        ) : (
          <table className="resumo-table">
            <tbody>
              {foreign && (
                <tr>
                  <td>
                    Resultado das vendas no ano
                    {Number(foreign.loss_after) > 0 && (
                      <div className="muted small">
                        Prejuízo para os próximos anos:{" "}
                        {money(foreign.loss_after)}
                      </div>
                    )}
                  </td>
                  <td className={`num ${tone(foreign.result)}`}>
                    <strong>{money(foreign.result)}</strong>
                  </td>
                </tr>
              )}
              {inc.exterior && (
                <tr>
                  <td>
                    Proventos recebidos lá fora
                    <div className="muted small">
                      Bruto {brl(inc.exterior.gross)}, com{" "}
                      {brl(inc.exterior.withheld)} retidos lá fora ·{" "}
                      {inc.exterior.assets.join(", ")}
                    </div>
                  </td>
                  <td className="num">
                    <strong>{brl(inc.exterior.gross)}</strong>
                  </td>
                </tr>
              )}
              <tr>
                <td>
                  Imposto estimado (15%), pago com a declaração
                  <div className="muted small">
                    Vendas: {foreign ? money(foreign.tax) : "R$ 0,00"} ·
                    proventos: 15% do bruto menos o retido lá fora
                  </div>
                </td>
                <td className="num">
                  <strong>
                    {brl(
                      Number(foreign?.tax ?? 0) +
                        (inc.exterior
                          ? Math.max(
                              0,
                              inc.exterior.gross * 0.15 - inc.exterior.withheld,
                            )
                          : 0),
                    )}
                  </strong>
                </td>
              </tr>
            </tbody>
          </table>
        )}
      </section>
      {print && (
        <p className="muted small">
          Gerado pelo 4Nier Patrimônio Auditável em{" "}
          {new Date().toLocaleDateString("pt-BR")}.
        </p>
      )}
    </div>
  );
}

export default function TaxSummary({
  refresh,
  onGo,
}: {
  refresh: number;
  onGo: (to: "proventos" | "impostos") => void;
}) {
  const [data, setData] = useState<Data | null>(null);
  const [suggestions, setSuggestions] = useState<SuggestedIncome[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [year, setYear] = useState<number | null>(null);

  useEffect(() => {
    Promise.all([
      taxApi.load(),
      incomesApi.load(),
      eventsApi.load(),
      market.portfolio().catch(() => null),
    ])
      .then(([view, inc, ev, pf]) =>
        setData({
          view,
          incomes: inc.items,
          events: ev.items,
          kinds: { ...(pf?.kinds ?? {}), ...inc.kinds },
          names: { ...(pf?.names ?? {}), ...inc.names, ...ev.names },
        }),
      )
      .catch((e) => setError(errorMessage(e)));
    // Proventos que faltam lançar vêm do histórico do mercado; sem internet, a lista fica vazia.
    incomesApi
      .market()
      .then((m) => setSuggestions(m.suggestions))
      .catch(() => setSuggestions([]));
  }, [refresh]);

  // Ano-base: o ano passado até o fim de maio (época da declaração), depois o atual.
  const years = useMemo(() => {
    if (!data) return [];
    const set = new Set<number>();
    data.view.months.forEach((m) => set.add(Number(m.month.slice(0, 4))));
    data.view.annual.foreign.forEach((y) => set.add(y.year));
    Object.keys(data.view.annual.positions).forEach((y) => set.add(Number(y)));
    data.view.savings_year_end.forEach((x) => set.add(x.year));
    data.incomes.forEach((r) => set.add(Number(r.income.pay_date.slice(0, 4))));
    return [...set].sort((a, b) => b - a);
  }, [data]);

  useEffect(() => {
    if (year != null || years.length === 0) return;
    const now = new Date();
    const base = now.getMonth() < 5 ? now.getFullYear() - 1 : now.getFullYear();
    setYear(years.includes(base) ? base : years[0]);
  }, [years, year]);

  const pend =
    data && year != null
      ? pendencias(
          year,
          data.view.months,
          suggestions,
          incomeLines(data.incomes, year).missingFx,
        )
      : [];

  return (
    <section className="taxes-page resumo">
      <div className="section-head">
        <div>
          <h2>Resumo para o contador</h2>
          <p className="muted">
            O que você lançou no ano, na ordem das fichas da declaração, para
            conferir com o contador. Os valores são os mesmos das outras telas.
            O aplicativo não emite DARF nem preenche a declaração.
          </p>
        </div>
        {years.length > 0 && (
          <label className="ret-pick">
            Ano-base
            <select
              value={String(year ?? "")}
              onChange={(e) => setYear(Number(e.target.value))}
            >
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
        {DISCLAIMER} O mesmo aviso sai no topo de cada página impressa.
      </p>

      {error && <p className="error">{error}</p>}
      {data && years.length === 0 && (
        <p className="muted">Ainda não há lançamentos para resumir.</p>
      )}

      {data && year != null && (
        <>
          <div className="resumo-actions">
            <button onClick={() => void taxApi.print()}>
              Imprimir ou salvar em PDF
            </button>
          </div>

          {pend.length > 0 && (
            <div className="banner warning resumo-pend" role="status">
              <b>Antes de levar ao contador, falta resolver:</b>
              <ul>
                {pend.map((p) => (
                  <li key={p.id}>
                    {p.text}
                    {p.action && (
                      <>
                        {" "}
                        <button
                          className="link"
                          onClick={() => onGo(p.action!)}
                        >
                          {p.action === "proventos" ? "Lançar" : "Ver"}
                        </button>
                      </>
                    )}
                  </li>
                ))}
              </ul>
            </div>
          )}

          <Body data={data} year={year} />

          {/* Versão para impressão: escondida na tela, é a única coisa que sai no papel. */}
          {createPortal(
            <div className="print-only">
              <table className="print-frame">
                <thead>
                  <tr>
                    <td>
                      <p className="print-disclaimer">{DISCLAIMER}</p>
                    </td>
                  </tr>
                </thead>
                <tbody>
                  <tr>
                    <td>
                      <h2>Resumo para o contador · ano-base {year}</h2>
                      {pend.length > 0 && (
                        <p className="small">
                          <b>Pendências:</b>{" "}
                          {pend.map((p) => p.text).join("; ")}.
                        </p>
                      )}
                      <Body data={data} year={year} print />
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>,
            document.body,
          )}
        </>
      )}
    </section>
  );
}
