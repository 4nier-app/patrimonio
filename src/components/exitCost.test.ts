import { describe, expect, it } from "vitest";
import { pctTyping, totalPct } from "./ExitCostBox";

describe("custo para trazer ao Brasil", () => {
  it("máscara de calculadora no percentual", () => {
    expect(pctTyping("1")).toBe("0,01");
    expect(pctTyping("100")).toBe("1,00");
    expect(pctTyping("1,005")).toBe("10,05");
    expect(pctTyping("0")).toBe("0,00");
    expect(pctTyping("")).toBe("");
    expect(pctTyping("123456")).toBe("12,34");
  });
  it("soma o IOF", () => {
    expect(totalPct("1")).toBeCloseTo(1.38);
  });
});
