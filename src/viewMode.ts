/**
 * Visão simples ou completa. A simples esconde as abas e colunas técnicas;
 * os cálculos são os mesmos. A escolha fica guardada neste computador (é uma
 * preferência de tela, não um dado da carteira).
 */
const KEY = "4nier.visao";

export function simpleView(): boolean {
  try {
    return localStorage.getItem(KEY) === "simples";
  } catch {
    return false;
  }
}

export function setSimpleView(on: boolean): void {
  try {
    localStorage.setItem(KEY, on ? "simples" : "completa");
  } catch {
    // Sem armazenamento: vale só até fechar o app.
  }
}

/** Abas que a visão simples esconde (continuam abrindo pelos atalhos das telas). */
export const ADVANCED_TABS = ["evolucao", "homebroker"] as const;
