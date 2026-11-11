import { useState } from "react";
import { errorMessage } from "../api";
import { saveXlsx, type XSheet } from "../planilha";

/** Botão "Exportar para Excel": pergunta onde salvar e grava a planilha. */
export default function ExportButton({ build, disabled }: { build: () => { name: string; sheets: XSheet[] }; disabled?: boolean }) {
  const [state, setState] = useState<"idle" | "busy" | "ok" | { error: string }>("idle");
  async function run() {
    setState("busy");
    try {
      const { name, sheets } = build();
      if (await saveXlsx(name, sheets)) {
        setState("ok");
        setTimeout(() => setState((s) => (s === "ok" ? "idle" : s)), 6000);
      } else setState("idle");
    } catch (e) {
      setState({ error: errorMessage(e) });
    }
  }
  return (
    <span className="export-xlsx">
      <button type="button" className="secondary" onClick={run} disabled={disabled || state === "busy"} title="Salva esta tabela numa planilha do Excel (.xlsx) no seu computador">
        {state === "busy" ? "Salvando…" : "Exportar para Excel"}
      </button>
      {state === "ok" && <span className="ok small">Planilha salva.</span>}
      {typeof state === "object" && <span className="error small">{state.error}</span>}
    </span>
  );
}
