import { Component, type ReactNode } from "react";

type Props = {
  children: ReactNode;
  onReset: () => void;
  /** Quando muda (outra aba ou outro ativo), a tela tenta desenhar de novo. */
  resetKey?: string;
};
type State = { error: Error | null };

/**
 * Se uma tela falhar ao desenhar, mostra o erro no lugar dela em vez de deixar
 * a janela toda em branco, e oferece voltar.
 */
/** Mensagem e as primeiras linhas de onde o erro aconteceu. */
export function describe(e: unknown): string {
  if (e instanceof Error) return [e.message, ...(e.stack ?? "").split("\n").slice(1, 6)].join("\n");
  return String(e);
}

export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidUpdate(prev: Props) {
    if (this.state.error && prev.resetKey !== this.props.resetKey) this.setState({ error: null });
  }

  componentDidCatch(error: Error) {
    console.error("Falha ao desenhar a tela:", error);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="card" role="alert">
        <h3>Esta tela teve um problema</h3>
        <p className="muted">Seus dados não foram afetados. Se acontecer de novo, mande esta mensagem para correção:</p>
        <pre className="error-detail">{describe(this.state.error)}</pre>
        <button
          onClick={() => {
            this.setState({ error: null });
            this.props.onReset();
          }}
        >
          Voltar
        </button>
      </div>
    );
  }
}
