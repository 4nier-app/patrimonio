// Marca 4Nier: o "4" desenhado como um gráfico subindo. Mesmo desenho do ícone
// do aplicativo (src-tauri/icons).
export function LogoMark({ size = 32 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 512 512" aria-hidden="true">
      <defs>
        <linearGradient id="logo-bg" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor="#0f2a4a" />
          <stop offset="1" stopColor="#0d6e6e" />
        </linearGradient>
      </defs>
      <rect width="512" height="512" rx="112" fill="url(#logo-bg)" />
      <g fill="none" stroke="#fff" strokeWidth="40" strokeLinecap="round" strokeLinejoin="round">
        <path d="M110 312 L222 182 L270 234 L340 122" />
        <path d="M110 312 H400" />
        <path d="M340 122 V392" />
      </g>
      <circle cx="340" cy="122" r="34" fill="#37d39b" />
    </svg>
  );
}

export default function Logo({ size = 32, subtitle = true }: { size?: number; subtitle?: boolean }) {
  return (
    <span className="logo">
      <LogoMark size={size} />
      <span className="logo-text">
        <span className="logo-name">
          <span className="logo-4">4</span>Nier
        </span>
        {subtitle && <span className="logo-sub">Carteira Auditável</span>}
      </span>
    </span>
  );
}
