// Formato comum que todo leitor de corretora devolve.

export type Side = "COMPRA" | "VENDA";

export type NoteLine = {
  side: Side;
  /** Código do ativo, quando a nota traz ou quando foi encontrado pelo nome. */
  ticker: string | null;
  /** Códigos possíveis quando o nome de pregão bate com mais de um. */
  candidates?: string[];
  /** Como o ativo aparece na nota ("PETROBRAS PN N2", "IBIT"). */
  title: string;
  /** Nome de pregão e classe, para achar o código quando a nota não traz. */
  lookup?: { name: string; cls: string };
  /** "VISTA", "FRACIONARIO" ou o que a nota disser. */
  market: string;
  /** Texto decimal com ponto: "100", "0.92681008". */
  quantity: string;
  price: string;
  /** Valor da linha na nota (quantidade × preço), com ponto. */
  value: string;
  /** Taxas desta linha: parte da nota, rateada pelo valor. */
  fees: string;
  dayTrade: boolean;
  /** Motivo para a linha vir desmarcada (não será importada sem o usuário marcar). */
  skip?: string;
};

export type NoteFee = { label: string; amount: string };

export type ParsedNote = {
  /** Leitor que entendeu a nota. */
  reader: string;
  /** Nome sugerido para a conta. */
  broker: string;
  currency: "BRL" | "USD";
  number: string | null;
  /** Data do pregão, AAAA-MM-DD. */
  date: string;
  lines: NoteLine[];
  /** Taxas como aparecem na nota. */
  fees: NoteFee[];
  /** Total de taxas e custos rateado entre as linhas. */
  totalFees: string;
  /** Conferência dos totais da nota; `ok = false` impede a importação. */
  check: { ok: boolean; text: string };
  warnings: string[];
  /**
   * Identidade da nota para a impressão digital (SHA-256): corretora, número
   * e data quando a nota tem número (a mesma nota pode vir em layouts
   * diferentes); senão, o texto das linhas.
   */
  identity: string;
};

export type Reader = {
  id: string;
  /** Confiança de 0 a 1 de que o documento é deste leitor. */
  detect(pages: string[][]): number;
  read(pages: string[][]): ParsedNote[];
};
