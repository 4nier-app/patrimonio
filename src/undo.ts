/** Aviso "Removido · Desfazer" que aparece por alguns segundos depois de remover. */
export type UndoOffer = { key: number; message: string; undo: () => Promise<void> };

let show: ((u: UndoOffer) => void) | null = null;
let seq = 0;

export function offerUndo(message: string, undo: () => Promise<void>): void {
  show?.({ key: ++seq, message, undo });
}

export function listenUndo(fn: ((u: UndoOffer) => void) | null): void {
  show = fn;
}
