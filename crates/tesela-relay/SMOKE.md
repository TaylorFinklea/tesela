# Relay smoke test

A two-mosaic, one-relay end-to-end test you can run on a single
machine. Validates that an edit on mosaic A propagates through the
relay to mosaic B without any LAN-discovery shortcut. If this works,
the whole pipeline (RelayClient + AEAD wrap + Axum relay + apply path)
is wired correctly on your system.

## Prereqs

- `cargo` available on $PATH
- `curl`, `jq` available on $PATH
- Repo checked out + workspace builds (`cargo build --workspace`)

## Run it

```sh
# From the repo root
crates/tesela-relay/scripts/smoke.sh
```

Expected output (abbreviated):

```
[smoke] relay listening on 127.0.0.1:18484
[smoke] server-a listening on 127.0.0.1:18001 (mosaic /tmp/tesela-smoke-a)
[smoke] server-b listening on 127.0.0.1:18002 (mosaic /tmp/tesela-smoke-b)
[smoke] paired b with a — both registered on the same group
[smoke] both servers registered with relay
[smoke] writing edit to mosaic A
[smoke] waiting ≤30s for edit to propagate to mosaic B via relay…
[smoke] ✓ edit visible in mosaic B (took 7s)
[smoke] OK: relay end-to-end works
```

If anything fails, the script leaves the relay + servers running for
inspection. `kill %1 %2 %3` or `pkill -f tesela-` to clean up. Or
just rerun — the script bails cleanly if old processes are bound to
the test ports.

## What the script does, step by step

1. **Start the relay** on port `18484` with an in-memory admin token.
2. **Initialize two fresh mosaics** (`/tmp/tesela-smoke-a` and
   `/tmp/tesela-smoke-b`) with their own `.tesela/config.toml` set up
   to point at the relay. Different bind ports (18001 / 18002) so
   they don't fight for `7474`.
3. **Start both servers** in the background. On boot each one
   registers with the relay (idempotent — second one sees the first's
   intent, recovers to the same `registered_at`, verifies).
4. **Pair them.** Asks server A for a pairing code, posts it to
   server B's `/sync/peer/pair-code` so they share a group identity.
   (LAN discovery is irrelevant here; the relay only knows about
   them by group id.)
5. **Write an edit.** `PUT /notes/smoke-test` against server A
   creates a tiny note.
6. **Poll server B** for the same note up to 30 s. The relay daemon
   on A picks up the local op + PUTs to the relay; daemon on B polls
   and applies; B's notes API serves it back.
7. **Verify the body matches.** If the bytes round-trip cleanly, the
   AEAD seal + relay storage + AEAD open all worked.

## Manual test, if you want to drive it yourself

Steps if you'd rather poke each piece individually:

```sh
# Terminal 1 — relay
cargo run -q --release --bin tesela-relay -- \
  --bind 127.0.0.1:18484 \
  --db /tmp/relay.sqlite \
  --admin-token smoke-admin-token

# Terminal 2 — server A
mkdir -p /tmp/tesela-smoke-a/.tesela /tmp/tesela-smoke-a/notes
cat > /tmp/tesela-smoke-a/.tesela/config.toml <<'EOF'
[server]
bind = "127.0.0.1:18001"

[sync.relay]
url = "http://127.0.0.1:18484"
poll_interval_ms = 2000
EOF
cargo run -q --release --bin tesela-server -- --mosaic /tmp/tesela-smoke-a

# Terminal 3 — server B (similar, with port 18002 + /tmp/tesela-smoke-b)

# Terminal 4 — orchestrate
# Pair B into A's group:
curl -s http://127.0.0.1:18001/sync/peer/pairing-code | jq -r .code | \
  xargs -I {} curl -s -X POST http://127.0.0.1:18002/sync/peer/pair-code \
    -H 'Content-Type: application/json' \
    -d "{\"code\":\"{}\"}"

# Write to A:
curl -s -X PUT http://127.0.0.1:18001/notes/smoke \
  -H 'Content-Type: application/json' \
  -d '{"content":"---\ntitle: \"smoke\"\n---\n\nhello from A"}'

# Wait + check B:
sleep 5
curl -s http://127.0.0.1:18002/notes/smoke | jq -r .body
# => "hello from A" (modulo formatting)
```

If the relay status endpoint is interesting:

```sh
curl -s http://127.0.0.1:18001/sync/relay/status | jq
curl -s http://127.0.0.1:18002/sync/relay/status | jq
```

You should see `inbound_cursor` ticking up + `last_poll_at` /
`last_put_at` recent.

## When it doesn't work

Common failure modes:

| Symptom | Likely cause | Fix |
|---|---|---|
| `relay: connection refused` in server logs | Relay didn't start | `cargo run -p tesela-relay` separately and confirm `GET /` returns OK |
| `relay registration intent does not verify` | Group keys differ on A and B | The pair step didn't happen, or B was started before being paired. Restart B after pairing. |
| Edits never appear on B | Relay daemon tick not firing | Check `RUST_LOG=tesela_server=debug,tesela_sync=debug` — should see "relay tick" messages every `poll_interval_ms` |
| 401 from MAC gate | Clock skew >300s between A/B | Sync your clocks (`sudo sntp -sS time.apple.com`) |
