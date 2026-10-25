import { describe, expect, it } from "vitest";
import { distribute, lines, type ClassId } from "./allocation";

const values = (v: Partial<Record<ClassId, number>>) =>
  ({ fundos: 0, b3: 0, fii: 0, exterior: 0, cripto: 0, rf: 0, fgts: 0, dinheiro: 0, ...v }) as Record<ClassId, number>;

describe("alocação ideal", () => {
  it("põe o aporte só em quem está abaixo da meta, sem vender", () => {
    // B3 60 (meta 50), renda fixa 40 (meta 50): os 20 vão todos para a renda fixa.
    const d = distribute(values({ b3: 60, rf: 40 }), { b3: 50, rf: 50 }, 20);
    expect(d).toEqual({ rf: 20 });
  });

  it("divide para todos chegarem juntos na meta quando o aporte basta", () => {
    // Total depois: 200. Metas 50/30/20 → 100/60/40.
    const d = distribute(values({ b3: 80, rf: 50, exterior: 30 }), { b3: 50, rf: 30, exterior: 20 }, 40);
    expect(d).toEqual({ b3: 20, rf: 10, exterior: 10 });
  });

  it("o FGTS conta no total, mas não recebe aporte", () => {
    const d = distribute(values({ fgts: 50, rf: 0 }), { fgts: 50, rf: 50 }, 30);
    expect(d).toEqual({ rf: 30 });
  });

  it("os centavos fecham com o valor do aporte", () => {
    const d = distribute(values({ b3: 0, rf: 0, fii: 0 }), { b3: 33.33, rf: 33.33, fii: 33.34 }, 100);
    expect(Object.values(d).reduce((s, v) => s + (v ?? 0), 0)).toBeCloseTo(100, 10);
  });

  it("mostra % de hoje e quanto falta só para quem tem meta", () => {
    const l = lines(values({ b3: 75, rf: 25, cripto: 10 }), { b3: 50, rf: 50 }, 0);
    const b3 = l.find((x) => x.id === "b3")!;
    expect(b3.share).toBe(75);
    expect(b3.gap).toBe(-25);
    expect(l.find((x) => x.id === "rf")!.gap).toBe(25);
    expect(l.find((x) => x.id === "cripto")!.share).toBeNull();
  });
});
