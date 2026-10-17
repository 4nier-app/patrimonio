/** Ano escolhido entre setas: ‹ 2026 ›. */
export default function YearPager({
  years,
  year,
  onChange,
  small,
}: {
  years: number[];
  year: number;
  onChange: (y: number) => void;
  small?: boolean;
}) {
  const list = [...years].sort((a, b) => a - b);
  const i = list.indexOf(year);
  return (
    <nav className={`segmented year-pager ${small ? "small" : ""}`} aria-label="Ano">
      <button type="button" className="seg arrow" disabled={i <= 0} onClick={() => onChange(list[i - 1])} aria-label="Ano anterior">
        ‹
      </button>
      <span className="seg year" aria-live="polite">
        {year}
      </span>
      <button
        type="button"
        className="seg arrow"
        disabled={i < 0 || i >= list.length - 1}
        onClick={() => onChange(list[i + 1])}
        aria-label="Próximo ano"
      >
        ›
      </button>
    </nav>
  );
}
