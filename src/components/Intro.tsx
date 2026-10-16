import type { ReactNode } from "react";

/** Explicação da tela: uma frase curta e, se houver, os detalhes em "Saiba mais". */
export default function Intro({ children, more }: { children: ReactNode; more?: ReactNode }) {
  return (
    <div className="intro">
      <p className="muted">{children}</p>
      {more && (
        <details className="help">
          <summary>Saiba mais</summary>
          <div className="muted">{more}</div>
        </details>
      )}
    </div>
  );
}
