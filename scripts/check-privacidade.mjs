#!/usr/bin/env node
// Garante que nenhuma nota de corretagem real entre no repositório.
// Só são aceitos PDFs dentro de fixtures/sinteticas/, que contém apenas notas
// fictícias geradas do zero. Ver docs/ARQUITETURA.md, seção 14.
import { execSync } from "node:child_process";
import { readFileSync } from "node:fs";

const files = execSync("git ls-files -z", { encoding: "utf8" }).split("\0").filter(Boolean);
const forbidden = files.filter(
  (f) => /\.(pdf|xlsx?|csv|ofx)$/i.test(f) && !f.startsWith("fixtures/sinteticas/"),
);

// CPF formatado (nnn.nnn.nnn-nn) em qualquer arquivo de texto versionado.
const cpf = /\b\d{3}\.\d{3}\.\d{3}-\d{2}\b/;
const withCpf = files.filter((f) => {
  if (/\.(png|ico|icns|pdf|lock)$/i.test(f) || f.startsWith("fixtures/sinteticas/")) return false;
  try {
    return cpf.test(readFileSync(f, "utf8"));
  } catch {
    return false;
  }
});

if (forbidden.length || withCpf.length) {
  for (const f of forbidden) console.error(`Arquivo de dados fora de fixtures/sinteticas/: ${f}`);
  for (const f of withCpf) console.error(`Possível CPF no arquivo: ${f}`);
  console.error("\nNotas e extratos reais nunca entram no repositório. Use notas sintéticas.");
  process.exit(1);
}
console.log(`Privacidade OK: ${files.length} arquivos verificados.`);
