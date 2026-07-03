# Venue Node-Role Taxonomy (RaceControl)

> **Purpose:** the canonical enumeration of venue **node roles** + the addressing convention. Authored 2026-07-04 to resolve a **dangling reference** — this file was cited as canonical by `venue-registry.json:2`, `VENUE-MODEL-AND-POD-MGMT-20260607.md:8`, and the rc-installer venue-type RCA, but had never been written. Companion to `venue-registry.json` (data) + `VENUE-INFRASTRUCTURE.md` (architecture).
>
> **Grounding discipline:** a role is **CANON** only if it appears in shipped code/config/registry. Roles below are tagged CANON or PROPOSAL. Do not treat a PROPOSAL role as canonical (that was a prior memory-projection error — `build_validation_host`/`venue_relay`/`spectator_display`-as-node-role had zero hits in either repo).

## Addressing convention
Address a node as **`<venue_id>/<role>`** (e.g. `rp-vlm/venue_heart`, `rp-vlm/pod-5`, `apex-chennai/venue_heart`). Say the **role**, not the machine/binding — `.23`/`.27`/`100.125.108.37`/`sim1-8` are the *rp-vlm binding*, not the vocabulary; a **pilot** ("bono") is never a node.

## CANON roles (grounded in code/registry)

| Role | Grounded in | What it is | Count/venue |
|---|---|---|---|
| **`venue_heart`** | `venue-registry.json` (`"role":"venue_heart"`); `VENUE-INFRASTRUCTURE.md` | The venue's server box. Runs the whole V3 heart stack: `rc-edge` (money authority, loopback `:8431`), `rc-gateway` (sole HMAC signer + reverse proxy + cockpit static, `:8432`/`:443`), Postgres `rc-pg16` (append-only durable store, `:5433`), `RCV3Watchdog`. The **only** node with a LAN-facing service port (the gateway). Also the **LAN jump-host to the pods** (esp. for sold `heart-exec`). | 1 |
| **`pod`** | `venue-registry.json` (`pods.pod-1..pod-N`) | A sim rig. Runs the pod agent (→ edge `/agent/*`) + `rc-sentry` (`:8091` exec). Reached per `pod_transport`: `tailscale-ssh` (own) or `heart-exec` (sold). | N (8 at rp-vlm) |
| **`control_node`** | `venue-registry.json` (`_transports` prose); `trusted_ssh_keys.rs` | The off-venue operator/HQ box (Bono VPS / operator machine) that runs `install-venue.sh` / `deploy-venue.sh` / `pod-mgmt` and reaches the venue per its transport. Not physically at the venue. | 1 (shared across venues) |

## PARTIAL / to-formalize

| Role | Status | Note |
|---|---|---|
| **`pos_terminal`** | PARTIAL | Appears only as `pos_terminal_id` in a W1-S6 email schema, not as a registry node-role. Reception PC; browser → gateway `/app/pos`. Formalize as a registry role if pods-style management is needed. |

## PROPOSAL roles (NOT canon — do not cite as existing)
These have **zero hits** in either repo; they are candidate roles only, kept here so the vocabulary is explicit but honestly scoped:
- `build_validation_host` — a native-Windows validation box (wine ≠ real Windows). Proposal.
- `venue_relay` — an in-venue relay/jump node. Proposal (today the `venue_heart` is the jump-host).
- `spectator_display` / `nvr` / `gateway`-as-node-role — **physical devices**, not V3 node-role identities.

## Physical devices at a venue (not V3 node-roles)
Present in the venue LAN but managed outside the V3 node model: `router`, `NVR + cameras` (Dahua), `spectator display`, PS5 consoles. Documented in the venue network map (`racecontrol/CLAUDE.md` Network Map / `comms-link/HALO-CATALOG-VENUE.md`), not addressed as `<venue_id>/<role>` yet.

## Own vs sold (transport axis — orthogonal to role)
Per `venue-registry.json` + the rc-installer venue-type RCA: `venue_type ∈ {own, sold}` selects `pod_transport` — own = `tailscale-ssh` (installer provisions OpenSSH), sold = `heart-exec` (installer removes OpenSSH; control_node → `venue_heart` `pod_exec` → `rc-sentry` → `pod`). Default `Sold` (SSH is explicit opt-in).
