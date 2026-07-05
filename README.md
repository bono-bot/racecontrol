# ⛔ OBSOLETE — RaceControl V2 (retired)

> **This repository is the retired V2 stack. Do not build new work here.**
>
> Active development is **V3** in [`bono-bot/rp-ecosystem`](https://github.com/bono-bot/rp-ecosystem).
> The rp-vlm venue cut over from V2 to V3 in July 2026 (see
> `rp-ecosystem` docs: `FLEET-MANIFEST-rp-vlm-20260703.md`, `RP-VLM-DEPLOY-RUNBOOK-20260702.md`).
> The V2 heart is frozen; pod-side `rc-agent` / `rc-sentry` remain only as inert legacy.

## What this repo is still useful for

- **Reference / archaeology only.** Design docs, decision ledgers, and contracts here
  describe behavior that V3 replaces or has not yet rebuilt.
- **Multiplayer (AC LAN lobby)** exists **only in V2** (`docs/LAUNCH-CONTRACT.md` §10,
  `lobby.rs`, `ac_server.rs`). V3 has no multiplayer orchestration yet — when it is built
  in `rp-ecosystem`, it should be a fresh design, not a port of this code.

## Status

- Marked obsolete: 2026-07-05 (Captain directive, chat session).
- Successor: [`bono-bot/rp-ecosystem`](https://github.com/bono-bot/rp-ecosystem) (V3, Rust workspace: `rc-edge`, `rc-cloud`, `rc-gateway`, `rc-contract`).
