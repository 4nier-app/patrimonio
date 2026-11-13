import { date } from "./format";

/**
 * Dia em que as regras fixas do app foram revisadas pela última vez: alíquotas
 * e limites do imposto, códigos e telas do programa do IR e o IOF do valor de
 * resgate. Elas vêm escritas no app, não da internet, então a tela mostra esta
 * data. A cada versão as regras são revisadas e a data acompanha a da versão
 * no CHANGELOG (o teste em revisao.test.ts confere).
 */
export const REGRAS_REVISADAS = "2026-11-13";

/** Frase curta para os avisos das telas. */
export function regrasRevisadas(): string {
  return `Regras revisadas em ${date(REGRAS_REVISADAS)}, a data desta versão do app: a lei pode ter mudado depois.`;
}
