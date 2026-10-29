import type { OperationOrigin } from "./api";
import { formatDateTime } from "./format";

/**
 * De onde veio um lançamento, em palavras: a nota de corretagem, a planilha
 * da B3, a planilha própria ou digitado na mão (sem registro de importação).
 */
export function originLabel(o: OperationOrigin | undefined): string {
  if (!o) return "Digitado";
  if (o.broker === "B3") return "Planilha da B3";
  if (o.broker === "Planilha") return "Planilha importada";
  return `Nota ${o.broker}${o.number ? ` nº ${o.number}` : ""}`;
}

export function originTitle(o: OperationOrigin | undefined): string {
  if (!o) return "Lançado na mão, nesta tela.";
  const when = `Importado em ${formatDateTime(o.imported_at)}.`;
  return o.edited ? `${when} Alterado depois da importação (veja em Segurança › Auditoria).` : when;
}

/** Origem de um grupo de lançamentos do mesmo dia. */
export function groupOrigin(list: (OperationOrigin | undefined)[]): string {
  const labels = [...new Set(list.map(originLabel))];
  if (labels.length === 1) return labels[0];
  if (labels.length === 2) return `${labels[0]} e ${labels[1][0].toLowerCase()}${labels[1].slice(1)}`;
  return "origens diferentes";
}
