/**
 * Tema claro ou escuro. "auto" segue o Windows; a escolha fica guardada neste
 * computador (é uma preferência de tela, não um dado da carteira).
 */
export type ThemePref = "auto" | "light" | "dark";

const KEY = "4nier.tema";
const QUERY = "(prefers-color-scheme: dark)";
const EVENT = "4nier-tema";

export function themePref(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    return v === "light" || v === "dark" ? v : "auto";
  } catch {
    return "auto";
  }
}

export function isDark(pref: ThemePref = themePref()): boolean {
  return pref === "auto" ? (window.matchMedia?.(QUERY).matches ?? false) : pref === "dark";
}

/** Aplica o tema na página e avisa os gráficos. */
export function applyTheme(): void {
  const dark = isDark();
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
  window.dispatchEvent(new Event(EVENT));
}

export function setThemePref(pref: ThemePref): void {
  try {
    if (pref === "auto") localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, pref);
  } catch {
    // Sem armazenamento, o tema vale só até fechar o app.
  }
  applyTheme();
}

/** Liga o tema ao abrir o app e acompanha a troca de tema do Windows. */
export function installTheme(): void {
  applyTheme();
  window.matchMedia?.(QUERY).addEventListener("change", applyTheme);
}

/** Chamado a cada troca de tema (escolha do usuário ou do Windows). */
export function onThemeChange(fn: () => void): () => void {
  window.addEventListener(EVENT, fn);
  return () => window.removeEventListener(EVENT, fn);
}
