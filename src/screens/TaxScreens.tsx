import { useEffect, useMemo, useState } from "react";
import {
  errorMessage,
  eventsApi,
  incomesApi,
  market,
  taxApi,
  type EventRow,
  type IncomeRow,
  type TaxView,
} from "../api";
import {
  bens,
  exclusivos,
  isentos,
  rendaVariavel,
  type Bem,
  type FiiMonth,
  type IncomeCode,
  type RvMonth,
} from "../telasIr";
import Intro from "../components/Intro";
import { regrasRevisadas } from "../revisao";

/**
 * Telas do IR: os valores do ano no mesmo formato das fichas do programa do
 * IR, para a pessoa copiar. Branco = o programa pede para digitar; cinza = o
 * programa calcula; contorno verde = valor que precisa ser digitado.
 */

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

const num = (v: number) =>
  v.toLocaleString("pt-BR", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

type Data = {
  view: TaxView;
  incomes: IncomeRow[];
  events: EventRow[];
  kinds: Record<string, string>;
  names: Record<string, string>;
};

type Ficha = "rv" | "isentos" | "exclusiva" | "bens";
const FICHAS: { id: Ficha; label: string }[] = [
  { id: "rv", label: "Renda Variável" },
  { id: "isentos", label: "Rendimentos Isentos e Não Tributáveis" },
  { id: "exclusiva", label: "Rendimentos Sujeitos à Tributação Exclusiva" },
  { id: "bens", label: "Bens e Direitos" },
];

/** Uma caixa do programa: digitada (branca) ou calculada (cinza). */
function Box({
  v,
  input,
  pct,
  text,
}: {
  v?: number;
  input?: boolean;
  pct?: boolean;
  text?: string;
}) {
  const shown = text ?? (pct ? `${num((v ?? 0) * 100).replace(",00", "")}%` : num(v ?? 0));
  const mark = input && (text ? text !== "" : (v ?? 0) !== 0);
  return (
    <span
      className={`irf-box ${input ? "in" : "calc"}${mark ? " mark" : ""}${(v ?? 0) < 0 ? " neg" : ""}${text != null ? " txt" : ""}`}
    >
      {shown}
    </span>
  );
}

function Legend({ extra }: { extra?: string }) {
  return (
    <div className="irf-legend">
      <span>
        <i className="in" />
        você digita no programa
      </span>
      <span>
        <i className="calc" />o programa calcula
      </span>
      <span>
        <i className="in mark" />
        digite este valor
      </span>
      {extra && <span>{extra}</span>}
    </div>
  );
}

// ---------- Renda Variável ----------

function Row({ label, c, d }: { label: string; c: React.ReactNode; d: React.ReactNode }) {
  return (
    <div className="irf-row">
      <span className="irf-lb">{label}</span>
      {c}
      {d}
    </div>
  );
}

function Grp({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="irf-grp">
      <div className="irf-gh">
        <span>⊟ {title}</span>
        <span className="irf-ch">Operações Comuns</span>
        <span className="irf-ch">Day-Trade</span>
      </div>
      {children}
    </div>
  );
}

const zeroRows = (labels: string[]) =>
  labels.map((l) => <Row key={l} label={l} c={<Box input />} d={<Box input />} />);

function Comuns({ months, year }: { months: RvMonth[]; year: number }) {
  const last = [...months].reverse().find((m) => m.hasSales);
  const [sel, setSel] = useState(last?.month ?? 1);
  useEffect(() => setSel(last?.month ?? 1), [year, last?.month]);
  const m = months[sel - 1];
  const jan = sel === 1;
  return (
    <div className="irf-frame">
      <div className="irf-months" role="tablist" aria-label="Mês">
        {months.map((x) => (
          <button
            key={x.month}
            role="tab"
            aria-selected={x.month === sel}
            className={`${x.month === sel ? "on" : ""}${x.hasSales ? " has" : ""}`}
            onClick={() => setSel(x.month)}
          >
            {MONTHS[x.month - 1]}
            {x.hasSales ? " •" : ""}
          </button>
        ))}
      </div>
      <div>
        {!m.hasSales && (
          <p className="irf-empty">
            Nenhuma venda em {MONTHS[sel - 1].toLowerCase()} de {year}. No programa do
            IR, não precisa digitar nada neste mês.
          </p>
        )}
        <fieldset className="irf-fs">
          <legend>Tipo de Mercado/Ativo</legend>
          <Grp title="Mercado à Vista">
            <Row
              label="Mercado à vista - ações"
              c={<Box input v={m.comum.result} />}
              d={<Box input v={m.dt.result} />}
            />
            {zeroRows(["Mercado à vista - ouro", "Mercado à vista - ouro at. fin. fora bolsa"])}
          </Grp>
          <Grp title="Mercado Opções">
            {zeroRows([
              "Mercado opções - ações",
              "Mercado opções - ouro",
              "Mercado opções - fora de bolsa",
              "Mercado opções - outros",
            ])}
          </Grp>
          <Grp title="Mercado Futuro">
            {zeroRows([
              "Mercado futuro - dólar dos EUA",
              "Mercado futuro - índices",
              "Mercado futuro - juros",
              "Mercado futuro - outros",
            ])}
          </Grp>
          <Grp title="Mercado a Termo">
            {zeroRows(["Mercado a termo - ações/ouro", "Mercado a termo - outros"])}
          </Grp>
          <Grp title="Resultados">
            <Row
              label="RESULTADO LÍQUIDO DO MÊS"
              c={<Box v={m.comum.result} />}
              d={<Box v={m.dt.result} />}
            />
            <Row
              label="Resultado negativo até o mês anterior"
              c={<Box input={jan} v={m.comum.lossBefore} />}
              d={<Box input={jan} v={m.dt.lossBefore} />}
            />
            <Row
              label="BASE DE CÁLCULO DO IMPOSTO"
              c={<Box v={m.comum.base} />}
              d={<Box v={m.dt.base} />}
            />
            <Row
              label="Prejuízo a compensar"
              c={<Box v={m.comum.lossAfter} />}
              d={<Box v={m.dt.lossAfter} />}
            />
            <Row
              label="Alíquota do imposto"
              c={<Box pct v={m.comum.rate} />}
              d={<Box pct v={m.dt.rate} />}
            />
            <Row label="IMPOSTO DEVIDO" c={<Box v={m.comum.tax} />} d={<Box v={m.dt.tax} />} />
          </Grp>
        </fieldset>
        <fieldset className="irf-fs">
          <legend>Consolidação do Mês</legend>
          {(
            [
              ["Total do imposto devido", m.totalTax, false],
              ["IR fonte de Day-Trade no mês", m.irrfDtMonth, true],
              ["IR fonte de Day-Trade nos meses anteriores", m.irrfDtBefore, false],
              ["IR fonte de Day-Trade a compensar", m.irrfDtComp, false],
              ["IR fonte (Lei nº 11.033/2004) no mês", m.irrfLeiMonth, true],
              ["IR fonte (Lei nº 11.033/2004) nos meses anteriores", m.irrfLeiBefore, false],
              ["IR fonte (Lei nº 11.033/2004) a compensar", m.irrfLeiComp, false],
              ["Imposto a pagar", m.toPay, false],
              ["Imposto pago", m.paid, true],
            ] as [string, number, boolean][]
          ).map(([l, v, input]) => (
            <Row key={l} label={l} c={<span />} d={<Box v={v} input={input} />} />
          ))}
        </fieldset>
      </div>
    </div>
  );
}

const FII_COLS: { label: string; get: (m: FiiMonth) => number; input?: (m: FiiMonth) => boolean; pct?: boolean }[] = [
  { label: "Resultado líquido do mês", get: (m) => m.result, input: () => true },
  {
    label: "Resultado negativo até o mês anterior",
    get: (m) => m.lossBefore,
    input: (m) => m.month === 1,
  },
  { label: "Base de cálculo do imposto", get: (m) => m.base },
  { label: "Prejuízo a compensar", get: (m) => m.lossAfter },
  { label: "Alíquota do imposto", get: (m) => m.rate * 100 },
  { label: "Imposto devido", get: (m) => m.tax },
  {
    label: "Saldo do imposto retido nos meses anteriores (Lei 11.033/2004)",
    get: (m) => m.irrfBefore,
  },
  { label: "Imposto retido no mês (Lei 11.033/2004)", get: (m) => m.irrfMonth, input: () => true },
  { label: "Imposto a compensar (Lei 11.033/2004)", get: (m) => m.irrfComp },
  { label: "Imposto a pagar", get: (m) => m.toPay },
  { label: "Imposto pago", get: (m) => m.paid, input: () => true },
];

function Fii({ months }: { months: FiiMonth[] }) {
  return (
    <div className="irf-frame block">
      <h4>Ganhos Líquidos ou Perdas</h4>
      <div className="irf-scroll">
        <table className="irf-table irf-fii">
          <thead>
            <tr>
              <th>Mês</th>
              {FII_COLS.map((c) => (
                <th key={c.label}>{c.label}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {months.map((m) => (
              <tr key={m.month} className={m.hasSales ? "has" : ""}>
                <th>
                  {MONTHS[m.month - 1].slice(0, 3).toUpperCase()}
                  {m.hasSales ? " •" : ""}
                </th>
                {FII_COLS.map((c) => {
                  const v = c.get(m);
                  const input = c.input?.(m) ?? false;
                  return (
                    <td
                      key={c.label}
                      className={`${input ? "in" : "calc"}${input && v !== 0 ? " mark" : ""}${v < 0 ? " neg" : ""}`}
                    >
                      {num(v)}
                    </td>
                  );
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function RendaVariavel({ data, year }: { data: Data; year: number }) {
  const rv = useMemo(() => rendaVariavel(data.view.months, year), [data, year]);
  const [tab, setTab] = useState<"comuns" | "fii">("comuns");
  const anyFii = rv.fii.some((m) => m.hasSales);
  return (
    <>
      <div className="irf-title">
        {tab === "comuns"
          ? "Renda Variável - Ganhos Líquidos ou Perdas em Operações Comuns/Day-Trade - Titular"
          : "Fundos de Investimento Imobiliário ou nas Cadeias Produtivas Agroindustriais - Titular"}
      </div>
      <div className="irf-tabs" role="tablist">
        <button role="tab" aria-selected={tab === "comuns"} onClick={() => setTab("comuns")}>
          Operações Comuns / Day-Trade
        </button>
        <button role="tab" aria-selected={tab === "fii"} onClick={() => setTab("fii")}>
          Operações em FII ou Fiagro
        </button>
      </div>
      {tab === "comuns" ? <Comuns months={rv.comuns} year={year} /> : <Fii months={rv.fii} />}
      <Legend extra="• mês com venda" />
      <p className="irf-tip">
        {tab === "comuns"
          ? `Estes valores vão na declaração de ${year + 1}, ano-base ${year}. Meses sem venda não precisam ser digitados. Ações, ETFs e BDRs entram em "Mercado à vista - ações".`
          : anyFii
            ? "Aqui os 12 meses ficam numa tabela só, como no programa."
            : `Nenhuma venda de FII ou Fiagro em ${year}: não precisa digitar nada nesta tela.`}{" "}
        O IR na fonte que sobrar em dezembro não passa para o ano seguinte: o programa usa no
        cálculo do imposto da declaração.
      </p>
    </>
  );
}

// ---------- Rendimentos ----------

function Rendimentos({
  title,
  codes,
  empty,
  note,
}: {
  title: string;
  codes: IncomeCode[];
  empty: string;
  note?: string;
}) {
  const [sel, setSel] = useState<string | null>(codes[0]?.id ?? null);
  useEffect(() => setSel(codes[0]?.id ?? null), [codes]);
  const c = codes.find((x) => x.id === sel) ?? codes[0];
  const total = codes.reduce((s, x) => s + x.total, 0);
  return (
    <>
      <div className="irf-title">{title}</div>
      {codes.length === 0 ? (
        <p className="irf-empty">{empty}</p>
      ) : (
        <>
          <table className="irf-table irf-list">
            <thead>
              <tr>
                <th>Tipo de Rendimento</th>
                <th className="v">Valor</th>
              </tr>
            </thead>
            <tbody>
              {codes.map((x) => (
                <tr
                  key={x.id}
                  className={`click${x.id === c.id ? " sel" : ""}`}
                  onClick={() => setSel(x.id)}
                >
                  <td>
                    <button className="irf-link" aria-pressed={x.id === c.id}>
                      {x.label}
                    </button>
                    {x.confirm && <span className="muted"> (confirme o código com o contador)</span>}
                  </td>
                  <td className="v calc">{num(x.total)}</td>
                </tr>
              ))}
              <tr className="tot">
                <td>Total</td>
                <td className="v">{num(total)}</td>
              </tr>
            </tbody>
          </table>
          <div className="irf-sub">
            <h5>{c.label} · o que digitar</h5>
            {c.totalOnly ? (
              <p className="small">
                Digite só o total do ano: <Box input v={c.total} />
              </p>
            ) : (
              <table className="irf-table">
                <thead>
                  <tr>
                    <th>Beneficiário</th>
                    <th>CNPJ da fonte pagadora</th>
                    <th>Nome da fonte pagadora</th>
                    <th className="v">Valor</th>
                  </tr>
                </thead>
                <tbody>
                  {c.payers.map((p) => (
                    <tr key={p.asset}>
                      <td>Titular</td>
                      <td className="na">veja no informe da corretora</td>
                      <td className="in mark">
                        {p.name && p.name !== p.asset ? `${p.name} (${p.asset})` : p.asset}
                      </td>
                      <td className={`v in${p.value !== 0 ? " mark" : ""}`}>{num(p.value)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
          {note && <p className="irf-tip">{note}</p>}
        </>
      )}
      <Legend />
    </>
  );
}

// ---------- Bens e Direitos ----------

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <>
      <span>{label}</span>
      <span>{children}</span>
    </>
  );
}

function BensDireitos({ data, year }: { data: Data; year: number }) {
  const list = useMemo(
    () => bens(data.view, data.kinds, { ...data.names, ...data.view.names }, year),
    [data, year],
  );
  const [sel, setSel] = useState<string | null>(null);
  const b: Bem | undefined = list.find((x) => x.key === sel) ?? list[0];
  const q = (v: string | null) => v ?? "?";
  const open = new Date() <= new Date(`${year}-12-31T23:59:59`);
  const end = open ? "hoje (prévia)" : `em 31/12/${year}`;
  return (
    <>
      <div className="irf-title">Declaração de Bens e Direitos</div>
      {open && (
        <p className="irf-empty">
          O ano de {year} ainda não terminou: a última coluna mostra a situação de hoje, como prévia.
          Na declaração vai a situação em 31/12/{year}.
        </p>
      )}
      {list.length === 0 ? (
        <p className="irf-empty">Nada na carteira em 31/12/{year - 1} nem {end}.</p>
      ) : (
        <>
          <div className="irf-scroll">
            <table className="irf-table irf-bens">
              <thead>
                <tr>
                  <th>Grupo</th>
                  <th>Código</th>
                  <th>Discriminação</th>
                  <th className="v">Situação em 31/12/{year - 1} (R$)</th>
                  <th className="v">Situação {end} (R$)</th>
                </tr>
              </thead>
              <tbody>
                {list.map((x) => (
                  <tr
                    key={x.key}
                    className={`click${x.key === b?.key ? " sel" : ""}`}
                    onClick={() => setSel(x.key)}
                  >
                    <td className={x.group ? "in mark" : "q"}>{q(x.group)}</td>
                    <td className={x.code ? "in mark" : "q"}>{q(x.code)}</td>
                    <td className="in mark">
                      <button className="irf-link" aria-pressed={x.key === b?.key}>
                        {x.discriminacao}
                      </button>
                    </td>
                    <td className={`v in${x.before ? " mark" : ""}`}>{num(x.before)}</td>
                    <td className={`v in${x.after ? " mark" : ""}`}>{num(x.after)}</td>
                  </tr>
                ))}
                <tr className="tot">
                  <td colSpan={3}>Total</td>
                  <td className="v">{num(list.reduce((s, x) => s + x.before, 0))}</td>
                  <td className="v">{num(list.reduce((s, x) => s + x.after, 0))}</td>
                </tr>
              </tbody>
            </table>
          </div>
          {b && (
            <div className="irf-sub">
              <h5>Bem selecionado · o que digitar</h5>
              <div className="irf-form">
                <Field label="Grupo / Código">
                  {b.group ? (
                    <Box input text={`${b.group} / ${b.code}`} />
                  ) : (
                    <span className="irf-box in txt">? confirme com o contador</span>
                  )}
                </Field>
                <Field label="Localização (País)">
                  <Box input text={b.country ?? "105 - Brasil"} />
                </Field>
                {b.ticker && (
                  <>
                    <Field label="CNPJ">
                      <span className="irf-box in txt na">veja no informe da corretora</span>
                    </Field>
                    <Field label="Negociados em bolsa?">
                      <Box input text="Sim" />
                    </Field>
                    <Field label="Código de negociação">
                      <Box input text={b.ticker} />
                    </Field>
                  </>
                )}
                <Field label="Discriminação">
                  <Box input text={b.discriminacao} />
                </Field>
                <Field label={`Situação em 31/12/${year - 1}`}>
                  <Box input v={b.before} />
                </Field>
                <Field label={`Situação ${end}`}>
                  <Box input v={b.after} />
                </Field>
                {b.profit != null && (
                  <>
                    <Field label="Lucro ou prejuízo (Lei 14.754)">
                      <Box input v={b.profit} />
                    </Field>
                    <Field label="Imposto pago no exterior">
                      <Box input v={0} />
                    </Field>
                  </>
                )}
              </div>
              <p className="irf-tip">
                Clique em um bem da tabela para ver o que digitar nele.
                {b.profit != null &&
                  " Nas aplicações no exterior, o programa pede o lucro ou prejuízo das vendas do ano e calcula sozinho o imposto de 15%. Valores em reais pela PTAX de cada data."}
              </p>
            </div>
          )}
        </>
      )}
      <Legend extra="? = grupo e código: confirme com o contador" />
    </>
  );
}

// ---------- tela ----------

export default function TaxScreens({ refresh }: { refresh: number }) {
  const [data, setData] = useState<Data | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [year, setYear] = useState<number | null>(null);
  const [ficha, setFicha] = useState<Ficha>("rv");

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
  }, [refresh]);

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

  // Ano-base: o ano passado até o fim de maio (época da declaração), depois o atual.
  useEffect(() => {
    if (year != null || years.length === 0) return;
    const now = new Date();
    const base = now.getMonth() < 5 ? now.getFullYear() - 1 : now.getFullYear();
    setYear(years.includes(base) ? base : years[0]);
  }, [years, year]);

  const names = data ? { ...data.names, ...data.view.names } : {};
  const isen = useMemo(
    () =>
      data && year != null
        ? isentos(data.incomes, data.events, data.view.months, names, year)
        : [],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [data, year],
  );
  const excl = useMemo(
    () => (data && year != null ? exclusivos(data.incomes, names, year) : []),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [data, year],
  );

  return (
    <section className="taxes-page telas-ir">
      <div className="section-head">
        <div>
          <h2>Telas do IR</h2>
          <Intro
            more={
              <>
                Branco é o campo que o programa do IR pede para você digitar; cinza é o que o
                próprio programa calcula; o contorno verde marca o valor que você precisa
                digitar. Em Renda Variável, ações, ETFs e BDRs entram em "Mercado à vista -
                ações", e FIIs e Fiagros têm a sua aba. O CNPJ de quem pagou os proventos não
                aparece porque não há fonte gratuita: veja no informe de rendimentos da
                corretora. Quando o grupo e o código de um bem não são certos, aparece "?":
                confirme com o contador. Em Bens e Direitos, enquanto o ano não termina, a
                última coluna mostra a situação de hoje como prévia (saldos informados e dólar
                pela PTAX de hoje); na declaração vai a de 31/12. Nada aqui é enviado para a
                Receita.
              </>
            }
          >
            Os valores do ano no mesmo formato das telas do programa do IR, ficha por ficha.
            São os mesmos valores das outras telas do app.
          </Intro>
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
        As telas abaixo seguem o programa do IR da Receita Federal de 2026, para ajudar você a
        copiar os valores. A Receita pode mudar as telas, os nomes e os códigos a cada ano,
        então no programa elas podem aparecer diferentes. Os valores são uma estimativa feita
        com o que você lançou no app: confira com um contador. O app não preenche a
        declaração nem emite DARF. {regrasRevisadas()}
      </p>

      {error && <p className="error">{error}</p>}
      {data && years.length === 0 && <p className="muted">Ainda não há lançamentos.</p>}

      {data && year != null && (
        <>
          <div className="irf-fichas" role="tablist" aria-label="Ficha">
            {FICHAS.map((f) => (
              <button
                key={f.id}
                role="tab"
                aria-selected={ficha === f.id}
                className={ficha === f.id ? "on" : ""}
                onClick={() => setFicha(f.id)}
              >
                {f.label}
              </button>
            ))}
          </div>
          <div className="card irf-card">
            {ficha === "rv" && <RendaVariavel data={data} year={year} />}
            {ficha === "isentos" && (
              <Rendimentos
                title="Rendimentos Isentos e Não Tributáveis"
                codes={isen}
                empty={`Nenhum rendimento isento em ${year}.`}
                note="Clique em cada tipo da tabela para ver o que digitar nele."
              />
            )}
            {ficha === "exclusiva" && (
              <Rendimentos
                title="Rendimentos Sujeitos à Tributação Exclusiva/Definitiva"
                codes={excl}
                empty={`Nenhum JCP ou juros em ${year}.`}
                note="Valor líquido, já descontado o imposto retido na fonte."
              />
            )}
            {ficha === "bens" && <BensDireitos data={data} year={year} />}
          </div>
        </>
      )}
    </section>
  );
}
