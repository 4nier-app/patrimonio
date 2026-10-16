// Extração do texto do PDF com o pdf.js (Mozilla), dentro do aplicativo.
// O arquivo não sai do computador.

import { getDocument, GlobalWorkerOptions, PasswordResponses } from "pdfjs-dist";
import workerUrl from "pdfjs-dist/build/pdf.worker.min.mjs?url";
import { buildLines, type TextPiece } from "./lines";

GlobalWorkerOptions.workerSrc = workerUrl;

export class PasswordNeeded extends Error {
  constructor(public wrong: boolean) {
    super(wrong ? "Senha do PDF incorreta." : "Este PDF tem senha.");
  }
}

/** Linhas de cada página. */
export async function pdfPages(data: Uint8Array, password?: string): Promise<string[][]> {
  const task = getDocument({ data, password });
  let doc;
  try {
    doc = await task.promise;
  } catch (e) {
    const err = e as { name?: string; code?: number };
    if (err?.name === "PasswordException")
      throw new PasswordNeeded(err.code === PasswordResponses.INCORRECT_PASSWORD);
    throw e;
  }
  const pages: string[][] = [];
  for (let i = 1; i <= doc.numPages; i++) {
    const page = await doc.getPage(i);
    const content = await page.getTextContent();
    const pieces: TextPiece[] = [];
    for (const it of content.items) {
      if (!("str" in it)) continue;
      pieces.push({ str: it.str, x: it.transform[4], y: it.transform[5], width: it.width, height: it.height });
    }
    pages.push(buildLines(pieces));
  }
  await task.destroy();
  return pages;
}
