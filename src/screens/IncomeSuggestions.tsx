import { useState } from "react";
import type { SuggestedIncome } from "../api";
import Intro from "../components/Intro";
import { date, money, price, quantity } from "../format";
import { KIND_LABEL } from "../incomes";
import { AssetName } from "./Portfolio";

export const suggestionKey = (s: SuggestedIncome) => `${s.account}|${s.asset}|${s.ex_date}`;

/** Lançamentos › Proventos › Proventos que faltam lançar (pelo histórico do mercado). */
export default function IncomeSuggestions({
  items,
  names,
  onFill,
  onLaunchAll,
}: {
  items: SuggestedIncome[];
  names: Record<string, string>;
  onFill: (s: SuggestedIncome) => void;
  onLaunchAll: () => Promise<void>;
}) {
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  if (!items.length) return null;
  const shown = open ? items : items.slice(0, 8);

  return (
    <div className="card stack spaced suggestions">
      <div>
        <h3>Proventos que faltam lançar ({items.length})</h3>
        <Intro
          more={
            <>
              O app cruza as cotas que você tinha em cada conta com o que cada ação, FII e ETF pagou por cota (pelo Yahoo Finance), desde a
              sua primeira compra. Aparecem só os que ainda não estão lançados. A data mostrada é a data ex. Nos EUA, o pagamento costuma cair 2 dias
              úteis depois, e o app já preenche assim; na B3, o prazo varia de dias a meses, então ajuste a data do pagamento antes de salvar,
              porque ela define o ano da declaração. Na B3, o Yahoo junta
              dividendos e JCP: se for JCP, troque o tipo e informe os 15% retidos. Nos EUA, já vem com os 30% retidos lá. Confira sempre
              com o extrato ou o informe de rendimentos da corretora.
            </>
          }
        >
          Pelo histórico dos ativos que você teve ou tem. Confira com o extrato da corretora e lance com um clique.
        </Intro>
      </div>
      <div className="scroll">
        <table>
          <thead>
            <tr>
              <th>Data ex</th>
              <th>Ativo</th>
              <th>Conta</th>
              <th className="num">Por cota</th>
              <th className="num">Cotas</th>
              <th className="num">Bruto</th>
              <th className="num">Retido</th>
              <th className="num">Líquido</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {shown.map((s) => {
              const net = (Number(s.gross) - Number(s.withheld)).toFixed(2);
              return (
                <tr key={suggestionKey(s)}>
                  <td>{date(s.ex_date)}</td>
                  <td>
                    <AssetName asset={s.asset} name={names[s.asset]} />
                    <div className="muted small">{KIND_LABEL[s.kind]}</div>
                  </td>
                  <td>{s.account}</td>
                  <td className="num">{price(s.per_share, s.currency)}</td>
                  <td className="num">{quantity(s.quantity)}</td>
                  <td className="num">{money(s.gross, s.currency)}</td>
                  <td className="num">{Number(s.withheld) ? money(s.withheld, s.currency) : "—"}</td>
                  <td className="num">{money(net, s.currency)}</td>
                  <td className="row-actions">
                    <button type="button" className="link" onClick={() => onFill(s)}>
                      Conferir e lançar
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="actions wrap">
        {items.length > 8 && (
          <button type="button" className="secondary" onClick={() => setOpen(!open)}>
            {open ? "Mostrar menos" : `Mostrar todos (${items.length})`}
          </button>
        )}
        <button
          type="button"
          className="secondary"
          disabled={busy}
          onClick={async () => {
            setBusy(true);
            try {
              await onLaunchAll();
            } finally {
              setBusy(false);
            }
          }}
        >
          {busy ? "Lançando…" : `Lançar todos (${items.length})`}
        </button>
        <span className="muted small">
          "Lançar todos" usa como data do pagamento a data ex mais 2 dias úteis nos EUA, e a própria data ex na B3; dá para alterar depois.
        </span>
      </div>
    </div>
  );
}
