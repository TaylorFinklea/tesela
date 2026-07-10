export type ContentJump = {
  id: number;
  noteId: string;
  query: string;
  snippet: string;
};

type ContentJumpRequest = Omit<ContentJump, "id">;

let nextJumpId = 0;
let pending = $state<ContentJump | null>(null);

export function requestContentJump(request: ContentJumpRequest): void {
  pending = { ...request, id: ++nextJumpId };
}

export function pendingContentJump(noteId: string): ContentJump | null {
  return pending?.noteId === noteId ? pending : null;
}

export function clearContentJump(id: number): void {
  if (pending?.id === id) pending = null;
}
