import log from "../CHANGELOG.md?raw";
import { describe, expect, it } from "vitest";
import { REGRAS_REVISADAS, regrasRevisadas } from "./revisao";

describe("data de revisão das regras fixas", () => {
  it("acompanha a data da versão mais recente do CHANGELOG", () => {
    const m = /^## \[[^\]]+\] - (\d{4}-\d{2}-\d{2})/m.exec(log);
    expect(m?.[1]).toBe(REGRAS_REVISADAS);
  });

  it("mostra a data no formato brasileiro", () => {
    expect(regrasRevisadas()).toContain("13/11/2026");
  });
});
