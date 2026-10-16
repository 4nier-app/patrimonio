// Leitura de notas de corretagem: escolhe o leitor da corretora pelo conteúdo.
// Para acrescentar uma corretora, crie um leitor em readers/ e inclua-o aqui.

import { nomadApex, nomadDriveWealth } from "./readers/nomad";
import { sinacor } from "./readers/sinacor";
import type { ParsedNote, Reader } from "./types";

export const READERS: Reader[] = [nomadDriveWealth, nomadApex, sinacor];

export class UnknownLayout extends Error {}

/** Notas contidas no documento (um PDF pode trazer várias). */
export function readNotes(pages: string[][]): ParsedNote[] {
  const ranked = READERS.map((r) => ({ r, score: r.detect(pages) })).sort((a, b) => b.score - a.score);
  if (!ranked.length || ranked[0].score < 0.5)
    throw new UnknownLayout("Não reconheci este arquivo como nota de corretagem de uma corretora conhecida.");
  return ranked[0].r.read(pages);
}

/** Impressão digital da nota (SHA-256 do texto), para não importar duas vezes. */
export async function noteHash(note: ParsedNote): Promise<string> {
  const bytes = new TextEncoder().encode(note.identity);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
}
