import { Fragment, useEffect, useState } from "react";
import Intro from "../components/Intro";
import { setShowMemos, useShowMemos } from "../memos";
import { api, errorMessage, type IntegrityReport, type LedgerEvent, type VaultStatus } from "../api";
import { formatDateTime, shortHash } from "../format";
import SystemHistory from "./SystemHistory";

export default function Audit({ status }: { status: VaultStatus }) {
  const [events, setEvents] = useState<LedgerEvent[]>([]);
  const [report, setReport] = useState<IntegrityReport | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.listEvents().then(setEvents).catch((e) => setError(errorMessage(e)));
  }, []);

  async function verify() {
    setReport(null);
    try {
      setReport(await api.verifyIntegrity());
    } catch (e) {
      setError(errorMessage(e));
    }
  }

  return (
    <section>
      <div className="section-head">
        <div>
          <h2>Auditoria</h2>
          <Intro more="Cada registro guarda o código (hash) do anterior, então qualquer adulteração quebra a corrente e aparece na verificação.">
            Toda inclusão, alteração ou remoção fica registrada aqui e não pode ser apagada.
          </Intro>
        </div>
        <button onClick={verify}>Verificar integridade</button>
      </div>

      <MemoToggle />

      {report && (
        <p className={report.ok ? "ok" : "error"}>
          {report.ok
            ? `Íntegro: ${report.events_checked} registros conferidos, nenhuma alteração encontrada.`
            : `Problema encontrado: ${report.problem}`}
        </p>
      )}
      {error && <p className="error">{error}</p>}

      <table>
        <thead>
          <tr>
            <th>#</th>
            <th>Data/hora</th>
            <th>Entidade</th>
            <th>Ação</th>
            <th>Hash</th>
          </tr>
        </thead>
        <tbody>
          {events.map((e) => (
            <Fragment key={e.seq}>
              <tr className="clickable" onClick={() => setOpen(open === e.seq ? null : e.seq)}>
                <td>{e.seq}</td>
                <td>{formatDateTime(e.occurred_at)}</td>
                <td>{e.entity} · {e.entity_id}</td>
                <td><span className={`badge ${e.action.toLowerCase()}`}>{e.action}</span></td>
                <td className="mono">{shortHash(e.hash)}</td>
              </tr>
              {open === e.seq && (
                <tr className="detail">
                  <td colSpan={5}>
                    <dl>
                      <dt>Antes</dt><dd className="mono">{e.before ?? "—"}</dd>
                      <dt>Depois</dt><dd className="mono">{e.after ?? "—"}</dd>
                      <dt>Motivo</dt><dd>{e.reason ?? "—"}</dd>
                      <dt>Versão do app</dt><dd>{e.app_version}</dd>
                      <dt>Hash anterior</dt><dd className="mono">{e.prev_hash}</dd>
                      <dt>Hash</dt><dd className="mono">{e.hash}</dd>
                    </dl>
                  </td>
                </tr>
              )}
            </Fragment>
          ))}
        </tbody>
      </table>
      <SystemHistory status={status} />
    </section>
  );
}

/** Liga ou desliga as memórias de cálculo do Painel e da Rentabilidade. */
function MemoToggle() {
  const on = useShowMemos();
  return (
    <div className="card memo-toggle">
      <label className="check">
        <input type="checkbox" checked={on} onChange={(e) => setShowMemos(e.target.checked)} />
        <strong>Mostrar memórias de cálculo</strong>
      </label>
      <p className="muted small">
        Com a opção ligada, ao clicar num resultado do Painel ou da Rentabilidade aparece a conta completa de como ele foi calculado. Desligada, os números
        são os mesmos, só sem os detalhes.
      </p>
    </div>
  );
}
