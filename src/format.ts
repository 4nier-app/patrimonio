const dateTime = new Intl.DateTimeFormat("pt-BR", { dateStyle: "short", timeStyle: "medium" });

/** Data/hora ISO (UTC) no fuso e formato do usuário. */
export function formatDateTime(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : dateTime.format(d);
}

/** Primeiros caracteres de um hash, para exibição. */
export function shortHash(hash: string, size = 10): string {
  return hash.length > size ? `${hash.slice(0, size)}…` : hash;
}

/** Força da senha numa escala de 0 a 4, para orientar o usuário. */
export function passwordStrength(password: string): number {
  let score = 0;
  if (password.length >= 8) score++;
  if (password.length >= 12) score++;
  if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score++;
  if (/\d/.test(password) && /[^A-Za-z0-9]/.test(password)) score++;
  return score;
}
