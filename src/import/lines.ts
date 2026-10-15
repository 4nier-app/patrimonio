// Monta linhas de texto a partir dos pedaços que o pdf.js devolve.
// Colunas ficam separadas por dois ou mais espaços, como no "pdftotext
// -layout", para que os leitores de cada corretora dividam por /\s{2,}/.

export type TextPiece = { str: string; x: number; y: number; width: number; height: number };

/** Linhas da página, de cima para baixo. */
export function buildLines(pieces: TextPiece[]): string[] {
  const items = pieces.filter((p) => p.str.trim() !== "");
  // Mesma linha: alturas parecidas (tolerância de 40% da fonte).
  const rows: { y: number; h: number; items: TextPiece[] }[] = [];
  for (const it of [...items].sort((a, b) => b.y - a.y)) {
    const h = it.height || 8;
    const row = rows.find((r) => Math.abs(r.y - it.y) <= Math.max(1.5, 0.4 * Math.min(h, r.h)));
    if (row) row.items.push(it);
    else rows.push({ y: it.y, h, items: [it] });
  }
  rows.sort((a, b) => b.y - a.y);
  return rows.map((r) => {
    const sorted = r.items.sort((a, b) => a.x - b.x);
    let out = "";
    let end = -Infinity;
    for (const it of sorted) {
      const charW = it.str.length ? it.width / it.str.length : 4;
      const gap = it.x - end;
      if (out) {
        if (gap > Math.max(charW * 1.8, 6)) out += "  ";
        else if (gap > charW * 0.25 && !out.endsWith(" ") && !it.str.startsWith(" ")) out += " ";
      }
      out += it.str;
      end = it.x + it.width;
    }
    return out.replace(/\s+$/, "");
  });
}
