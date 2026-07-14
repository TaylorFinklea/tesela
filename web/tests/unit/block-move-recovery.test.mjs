import { test } from "node:test";
import { strict as assert } from "node:assert";

import { BlockMoveRecoveryOwner } from "../../src/lib/block-move-recovery.ts";

const request = {
  move_id: "11111111-1111-4111-8111-111111111111",
  source_note_id: "2026-07-13",
  root_bid: "22222222-2222-4222-8222-222222222222",
  destination_note_id: "2026-07-12",
  target_bid: "33333333-3333-4333-8333-333333333333",
  placement: "after",
};

class MemoryStorage {
  values = new Map();
  throwOnRemove = false;

  getItem(key) {
    return this.values.get(key) ?? null;
  }

  setItem(key, value) {
    this.values.set(key, value);
  }

  removeItem(key) {
    if (this.throwOnRemove) throw new Error("remove failed");
    this.values.delete(key);
  }
}

class ReservationBarrier {
  calls = [];
  reservations = [];

  reserve(noteIds) {
    this.calls.push([...noteIds]);
    const reservation = {
      released: false,
      settle: async () => {},
      release() {
        this.released = true;
      },
    };
    this.reservations.push(reservation);
    return reservation;
  }
}

test("submitted move ownership persists the exact request before transport state is published", () => {
  const storage = new MemoryStorage();
  const barrier = new ReservationBarrier();
  const owner = new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");
  const reservation = barrier.reserve([request.source_note_id, request.destination_note_id]);
  const states = [];
  owner.subscribe((state) => states.push(state?.status ?? "idle"));

  owner.adopt(request, reservation);

  const stored = JSON.parse([...storage.values.values()][0]);
  assert.deepEqual(stored, {
    version: 1,
    scope: "https://tesela.test/api",
    request,
  });
  assert.deepEqual(owner.current(), {
    request,
    status: "submitting",
    message: null,
    blockingMoveId: null,
  });
  assert.deepEqual(states, ["idle", "submitting"]);
  assert.equal(reservation.released, false);
});

test("reload rehydrates an exact submitted request as retryable and reacquires both notes", () => {
  const storage = new MemoryStorage();
  storage.setItem("tesela:block-move-recovery:v1", JSON.stringify({
    version: 1,
    scope: "https://tesela.test/api",
    request,
  }));
  const barrier = new ReservationBarrier();

  const owner = new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");

  assert.deepEqual(barrier.calls, [[request.source_note_id, request.destination_note_id]]);
  assert.equal(owner.current()?.status, "retryable");
  assert.deepEqual(owner.current()?.request, request);

  assert.equal(owner.markSubmitting(request.move_id), true);
  assert.equal(owner.current()?.status, "submitting");
  assert.equal(owner.markSubmitting(request.move_id), false, "a retry is single-flight");
  assert.equal(owner.complete(request.move_id), true);
  assert.equal(owner.current(), null);
  assert.equal(storage.values.size, 0);
  assert.equal(barrier.reservations[0].released, true);
});

test("foreign move ids cannot alter or release the owned exact request", () => {
  const storage = new MemoryStorage();
  const barrier = new ReservationBarrier();
  const owner = new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");
  const reservation = barrier.reserve([request.source_note_id, request.destination_note_id]);
  owner.adopt(request, reservation);

  assert.equal(owner.markRetryable("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa", "foreign", null), false);
  assert.equal(owner.complete("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"), false);
  assert.equal(owner.current()?.request.move_id, request.move_id);
  assert.equal(reservation.released, false);

  assert.equal(
    owner.markRetryable(
      request.move_id,
      "The server is recovering an earlier move",
      "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    ),
    true,
  );
  assert.equal(owner.current()?.blockingMoveId, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
  assert.equal(owner.current()?.status, "retryable");
});

test("invalid persisted requests are discarded without reserving notes", () => {
  const storage = new MemoryStorage();
  storage.setItem("tesela:block-move-recovery:v1", JSON.stringify({
    version: 1,
    scope: "https://tesela.test/api",
    request: { ...request, root_bid: "not-a-uuid" },
  }));
  const barrier = new ReservationBarrier();

  const owner = new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");

  assert.equal(owner.current(), null);
  assert.deepEqual(barrier.calls, []);
  assert.equal(storage.values.size, 0);
});

test("unavailable persistence fails closed without stealing the caller's reservation", () => {
  const barrier = new ReservationBarrier();
  const owner = new BlockMoveRecoveryOwner(barrier, null, "https://tesela.test/api");
  const reservation = barrier.reserve([request.source_note_id, request.destination_note_id]);

  assert.throws(() => owner.adopt(request, reservation), /storage is unavailable/i);
  assert.equal(owner.current(), null);
  assert.equal(reservation.released, false);
});

test("a recovery marker from another API scope is discarded", () => {
  const storage = new MemoryStorage();
  storage.setItem("tesela:block-move-recovery:v1", JSON.stringify({
    version: 1,
    scope: "https://other.test/api",
    request,
  }));
  const barrier = new ReservationBarrier();

  const owner = new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");

  assert.equal(owner.current(), null);
  assert.deepEqual(barrier.calls, []);
  assert.equal(storage.values.size, 0);
});

test("same-note recovery reacquires one deduplicated reservation", () => {
  const sameNoteRequest = {
    ...request,
    destination_note_id: request.source_note_id,
    target_bid: null,
    placement: "append",
  };
  const storage = new MemoryStorage();
  storage.setItem("tesela:block-move-recovery:v1", JSON.stringify({
    version: 1,
    scope: "https://tesela.test/api",
    request: sameNoteRequest,
  }));
  const barrier = new ReservationBarrier();

  new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");

  assert.deepEqual(barrier.calls, [[request.source_note_id]]);
});

test("terminal cleanup stays frozen when its durable marker cannot be removed", () => {
  const storage = new MemoryStorage();
  const barrier = new ReservationBarrier();
  const owner = new BlockMoveRecoveryOwner(barrier, storage, "https://tesela.test/api");
  const reservation = barrier.reserve([request.source_note_id, request.destination_note_id]);
  owner.adopt(request, reservation);
  storage.throwOnRemove = true;

  assert.equal(owner.complete(request.move_id), false);
  assert.equal(owner.current()?.status, "retryable");
  assert.match(owner.current()?.message ?? "", /could not be cleared/i);
  assert.equal(reservation.released, false);
});
