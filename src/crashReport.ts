/**
 * Último recurso contra a janela em branco: qualquer erro que escape das telas
 * aparece numa faixa no rodapé, com um botão para recarregar. Feito sem React,
 * para funcionar mesmo se o React parar.
 */
export function installCrashReport() {
  const show = (what: string) => {
    let box = document.getElementById("crash-report");
    if (!box) {
      box = document.createElement("div");
      box.id = "crash-report";
      box.setAttribute("role", "alert");
      box.innerHTML =
        '<strong>O aplicativo teve um problema.</strong> Seus dados não foram afetados. Mande um print desta faixa para correção.' +
        '<pre></pre><button type="button">Recarregar a tela</button>';
      box.querySelector("button")!.addEventListener("click", () => window.location.reload());
      document.body.appendChild(box);
    }
    const pre = box.querySelector("pre")!;
    pre.textContent = `${pre.textContent ? pre.textContent + "\n\n" : ""}${what}`.slice(-4000);
    // Se o React desmontou tudo, a raiz fica vazia: avisa também disso.
    if (!document.getElementById("root")?.childElementCount) pre.textContent += "\n(tela vazia)";
  };
  const text = (e: unknown) => (e instanceof Error ? `${e.message}\n${(e.stack ?? "").split("\n").slice(1, 6).join("\n")}` : String(e));
  window.addEventListener("error", (ev) => {
    // ResizeObserver avisa laços de redimensionamento inofensivos.
    if (String(ev.message).includes("ResizeObserver")) return;
    show(text(ev.error ?? ev.message));
  });
  window.addEventListener("unhandledrejection", (ev) => {
    // Erros de comandos do aplicativo já aparecem nas telas.
    if (typeof ev.reason === "string") return;
    show(text(ev.reason));
  });
}
