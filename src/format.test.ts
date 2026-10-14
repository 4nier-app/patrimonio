import { describe, expect, it } from "vitest";
import { passwordStrength, shortHash } from "./format";

describe("shortHash", () => {
  it("encurta hashes longos", () => {
    expect(shortHash("abcdef0123456789", 6)).toBe("abcdef…");
  });
  it("mantém textos curtos", () => {
    expect(shortHash("abc")).toBe("abc");
  });
});

describe("passwordStrength", () => {
  it("vai de fraca a forte", () => {
    expect(passwordStrength("abc")).toBe(0);
    expect(passwordStrength("abcdefgh")).toBe(1);
    expect(passwordStrength("Abcdefgh1234!")).toBe(4);
  });
});
