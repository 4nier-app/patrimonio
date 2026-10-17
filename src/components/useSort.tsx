import { useMemo, useState } from "react";

type Value = number | string | null | undefined;
type Sort = { key: string; dir: 1 | -1 } | null;

/**
 * Ordenação por clique no título da coluna. Primeiro clique: números do maior
 * para o menor (datas, da mais recente), textos de A a Z; segundo inverte; terceiro volta à ordem
 * original. Vazios ficam sempre no fim. A escolha fica guardada por tabela.
 */
export function useSort<T>(rows: T[], columns: Record<string, (r: T) => Value>, storageKey: string) {
  const [sort, setSortState] = useState<Sort>(() => {
    try {
      const v = JSON.parse(localStorage.getItem(`4nier.ordem.${storageKey}`) ?? "null");
      return v && typeof v.key === "string" && columns[v.key] && (v.dir === 1 || v.dir === -1) ? v : null;
    } catch {
      return null;
    }
  });

  function setSort(s: Sort) {
    setSortState(s);
    try {
      if (s) localStorage.setItem(`4nier.ordem.${storageKey}`, JSON.stringify(s));
      else localStorage.removeItem(`4nier.ordem.${storageKey}`);
    } catch {
      // Sem armazenamento, a ordem vale só nesta tela.
    }
  }

  const sorted = useMemo(() => {
    if (!sort || !columns[sort.key]) return rows;
    const get = columns[sort.key];
    return rows
      .map((r, i) => ({ r, i, v: get(r) }))
      .sort((a, b) => {
        const empty = (a.v == null || a.v === "" ? 1 : 0) - (b.v == null || b.v === "" ? 1 : 0);
        if (empty) return empty;
        if (a.v == null || b.v == null) return a.i - b.i;
        const c = typeof a.v === "number" && typeof b.v === "number" ? a.v - b.v : String(a.v).localeCompare(String(b.v), "pt-BR");
        return c * sort.dir || a.i - b.i;
      })
      .map((x) => x.r);
    // As colunas são recriadas a cada render; a ordem depende só da chave.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rows, sort]);

  function header(key: string, label: React.ReactNode, opts: { numeric?: boolean; desc?: boolean; title?: string } = {}) {
    const active = sort?.key === key;
    const first: 1 | -1 = opts.numeric || opts.desc ? -1 : 1;
    function next() {
      if (!active) setSort({ key, dir: first });
      else if (sort!.dir === first) setSort({ key, dir: (-first) as 1 | -1 });
      else setSort(null);
    }
    // Nas colunas de números a seta fica à esquerda, para o título continuar
    // alinhado à direita com os valores.
    const arrow = (
      <span className="sort-arrow" aria-hidden="true">
        {active ? (sort!.dir === 1 ? "▲" : "▼") : "↕"}
      </span>
    );
    return (
      <th
        className={`${opts.numeric ? "num " : ""}sortable${active ? " sorted" : ""}`}
        aria-sort={!active ? "none" : sort!.dir === 1 ? "ascending" : "descending"}
        title={opts.title}
      >
        <button type="button" className="sort-btn" onClick={next}>
          {opts.numeric && arrow}
          {label}
          {!opts.numeric && arrow}
        </button>
      </th>
    );
  }

  return { sorted, header };
}
