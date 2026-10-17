import { useEffect, useState } from "react";
import { errorMessage } from "../api";
import { listenUndo, type UndoOffer } from "../undo";

const SECONDS = 10;

export default function UndoToast() {
  const [offer, setOffer] = useState<UndoOffer | null>(null);
  const [state, setState] = useState<"idle" | "busy" | "done" | string>("idle");

  useEffect(() => {
    listenUndo((u) => {
      setOffer(u);
      setState("idle");
    });
    return () => {
      listenUndo(null);
    };
  }, []);

  useEffect(() => {
    if (!offer) return;
    const t = setTimeout(() => setOffer(null), (state === "idle" ? SECONDS : 3) * 1000);
    return () => {
      clearTimeout(t);
    };
  }, [offer, state]);

  if (!offer) return null;

  async function undo() {
    if (!offer) return;
    setState("busy");
    try {
      await offer.undo();
      setState("done");
    } catch (e) {
      setState(`Não consegui desfazer: ${errorMessage(e)}`);
    }
  }

  return (
    <div className="toast" role="status" aria-live="polite">
      {state === "done" ? (
        <span>Pronto, desfeito.</span>
      ) : state !== "idle" && state !== "busy" ? (
        <span>{state}</span>
      ) : (
        <>
          <span>{offer.message}</span>
          <button type="button" onClick={undo} disabled={state === "busy"}>
            {state === "busy" ? "Desfazendo…" : "Desfazer"}
          </button>
        </>
      )}
      <button type="button" className="toast-close" onClick={() => setOffer(null)} aria-label="Fechar">
        ✕
      </button>
    </div>
  );
}
