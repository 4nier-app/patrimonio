import { Component, type ReactNode } from "react";

type Props = { children: ReactNode; onReset: () => void };
type State = { error: Error | null };

/**
 * Se uma tela falhar ao desenhar, mostra o erro no lugar dela em vez de deixar
 * a janela toda em branco, e oferece voltar.
 */
export default class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
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
        <pre className="error-detail">{String(this.state.error?.message ?? this.state.error)}</pre>
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
