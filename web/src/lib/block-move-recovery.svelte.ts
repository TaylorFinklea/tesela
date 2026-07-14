import { blockMoveMutationBarrier } from "./block-ops-saver.ts";
import {
  BlockMoveRecoveryOwner,
  type BlockMoveRecoveryStorage,
} from "./block-move-recovery.ts";
import { apiBase } from "./runtime-base.ts";

function recoveryStorage(): BlockMoveRecoveryStorage | null {
  if (typeof window === "undefined") return null;
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

export const blockMoveRecovery = new BlockMoveRecoveryOwner(
  blockMoveMutationBarrier,
  recoveryStorage(),
  typeof window === "undefined"
    ? "server"
    : new URL(apiBase(), window.location.origin).href.replace(/\/$/, ""),
);
