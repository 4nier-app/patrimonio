import { useCallback, useEffect, useState } from "react";
import { api, errorMessage, type VaultStatus } from "./api";
import Login from "./screens/Login";
import Home from "./screens/Home";

export default function App() {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    api.vaultStatus().then(setStatus).catch((e) => setError(errorMessage(e)));
  }, []);

  useEffect(refresh, [refresh]);

  if (error) return <main className="center"><p className="error">{error}</p></main>;
  if (!status) return <main className="center"><p className="muted">Carregando…</p></main>;
  if (!status.unlocked) return <Login status={status} onUnlocked={refresh} />;
  return <Home status={status} onLocked={refresh} onChanged={refresh} />;
}
