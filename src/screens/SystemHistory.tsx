import { useEffect, useState } from "react";
import { api, errorMessage, type VaultStatus, type VersionEntry } from "../api";
import { formatDateTime } from "../format";

export default function SystemHistory({ status }: { status: VaultStatus }) {
  const [entries, setEntries] = useState<VersionEntry[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.versionHistory().then(setEntries).catch((e) => setError(errorMessage(e)));
  }, []);

  return (
    <section className="versions">
      <h2>Versões do aplicativo</h2>
      <p className="muted">
        Versão em uso: {status.app_version}. Cada versão fica registrada com a data em que foi instalada e o que mudou
        nela, para saber com qual regra de cálculo cada resultado foi gerado.
      </p>
      {error && <p className="error">{error}</p>}
      {entries.map((v) => (
        <article key={v.version} className="card version">
          <h3>Versão {v.version}</h3>
          <p className="muted">Instalada em {formatDateTime(v.installed_at)}</p>
          <pre>{v.changelog}</pre>
        </article>
      ))}
    </section>
  );
}
