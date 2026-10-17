import { useEffect, useState } from "react";

/**
 * Mostrar ou não as memórias de cálculo (Painel, Rentabilidade). É uma
 * preferência de tela, guardada neste computador; os números não mudam.
 */
const KEY = "4nier.memorias";
const EVENT = "4nier-memorias";

/** Desligada por padrão: só aparece para quem ligar em Segurança › Auditoria. */
export function showMemos(): boolean {
  try {
    return localStorage.getItem(KEY) === "sim";
  } catch {
    return false;
  }
}

export function setShowMemos(on: boolean): void {
  try {
    if (on) localStorage.setItem(KEY, "sim");
    else localStorage.removeItem(KEY);
  } catch {
    // Sem armazenamento, a escolha vale só até fechar o app.
  }
  window.dispatchEvent(new Event(EVENT));
}

export function useShowMemos(): boolean {
  const [on, setOn] = useState(showMemos);
  useEffect(() => {
    const fn = () => setOn(showMemos());
    window.addEventListener(EVENT, fn);
    return () => {
      window.removeEventListener(EVENT, fn);
    };
  }, []);
  return on;
}
