// Números das notas como texto decimal exato (sem ponto flutuante).

/** "1.234,56" ou "-R$ 1.234,56" → "1234.56" (sinal mantido). */
export function brNumber(raw: string): string | null {
  const s = raw.replace(/R\$|\s/g, "");
  const m = /^(-?)(\d{1,3}(?:\.\d{3})*|\d+)(?:,(\d+))?$/.exec(s);
  if (!m) return null;
  const int = m[2].replace(/\./g, "").replace(/^0+(?=\d)/, "");
  return `${m[1]}${int}${m[3] ? "." + m[3] : ""}`;
}

/** "1,234.56", "$1,234.56" ou "-0.00000774" → "1234.56". */
export function usNumber(raw: string): string | null {
  const s = raw.replace(/\$|\s/g, "");
  const m = /^(-?)(\d{1,3}(?:,\d{3})*|\d+)(?:\.(\d+))?$/.exec(s);
  if (!m) return null;
  const int = m[2].replace(/,/g, "").replace(/^0+(?=\d)/, "");
  return `${m[1]}${int}${m[3] ? "." + m[3] : ""}`;
}

// Aritmética em centavos (inteiros grandes) para totais e rateio.

/** "12.345" → 1234n centavos, truncando o que passar de 2 casas. */
export function toCents(dec: string): bigint {
  const neg = dec.startsWith("-");
  const [i, f = ""] = dec.replace("-", "").split(".");
  const c = BigInt(i || "0") * 100n + BigInt((f + "00").slice(0, 2));
  return neg ? -c : c;
}

export function fromCents(c: bigint): string {
  const neg = c < 0n;
  const a = neg ? -c : c;
  const s = `${a / 100n}.${String(a % 100n).padStart(2, "0")}`;
  return neg ? `-${s}` : s;
}

/** Quantidade × preço, arredondado ao centavo (meio para cima). */
export function grossCents(quantity: string, price: string): bigint {
  const [qi, qf = ""] = quantity.replace("-", "").split(".");
  const [pi, pf = ""] = price.replace("-", "").split(".");
  const q = BigInt(qi + qf);
  const p = BigInt(pi + pf);
  const scale = qf.length + pf.length;
  const prod = q * p; // escala 10^scale
  if (scale <= 2) return prod * 10n ** BigInt(2 - scale);
  const div = 10n ** BigInt(scale - 2);
  return (prod + div / 2n) / div;
}

/**
 * Rateia `total` centavos entre as linhas na proporção de `weights`,
 * truncando cada parte; o resto vai para a linha de maior peso.
 */
export function allocate(total: bigint, weights: bigint[]): bigint[] {
  const sum = weights.reduce((a, b) => a + b, 0n);
  if (weights.length === 0) return [];
  if (sum === 0n) return weights.map((_, i) => (i === 0 ? total : 0n));
  const parts = weights.map((w) => (total * w) / sum);
  const rest = total - parts.reduce((a, b) => a + b, 0n);
  let big = 0;
  weights.forEach((w, i) => {
    if (w > weights[big]) big = i;
  });
  parts[big] += rest;
  return parts;
}

/** "08/05/2026" → "2026-05-08". */
export function brDate(s: string): string | null {
  const m = /^(\d{2})\/(\d{2})\/(\d{4})$/.exec(s.trim());
  return m ? `${m[3]}-${m[2]}-${m[1]}` : null;
}

/** "4/3/2025" (mês/dia/ano) → "2025-04-03". */
export function usDate(s: string): string | null {
  const m = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(s.trim());
  return m ? `${m[3]}-${m[1].padStart(2, "0")}-${m[2].padStart(2, "0")}` : null;
}
