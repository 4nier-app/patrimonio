import { describe, expect, it } from "vitest";
import { groupOrigin, originLabel } from "./origin";
import type { OperationOrigin } from "./api";

const o = (broker: string, number: string | null = null): OperationOrigin => ({
  operation_id: "x",
  broker,
  number,
  imported_at: "2026-01-02T10:00:00Z",
  edited: false,
});

describe("origem do lançamento", () => {
  it("diz de onde veio", () => {
    expect(originLabel(undefined)).toBe("Digitado");
    expect(originLabel(o("B3"))).toBe("Planilha da B3");
    expect(originLabel(o("Planilha"))).toBe("Planilha importada");
    expect(originLabel(o("Corretora Teste", "123"))).toBe("Nota Corretora Teste nº 123");
  });
  it("resume o grupo do dia", () => {
    expect(groupOrigin([o("B3"), o("B3")])).toBe("Planilha da B3");
    expect(groupOrigin([o("B3"), undefined])).toBe("Planilha da B3 e digitado");
  });
});
