# MMA Step 1 DIAGNOSE — Venue-type SSH provisioning (2026-06-07)

**Gate:** §S-146 foundational-boundary escalation (pod auth/access) → MMA before PLAN.
**RCA under review:** `RCA-VENUE-TYPE-SSH-PROVISIONING-20260607.md`.
**Run:** `/tmp/mma-diagnose-venue-ssh.mjs` · 5 models / 5 vendors (deepseek-r1, qwen3-coder, nvidia-nemotron, gemini-2.5-pro, kimi-k2.5) · 5/5 OK · $0.05097 · spend-ledger `openrouter-spend-bono.jsonl` surface `MMA-DIAGNOSE-venue-ssh-bono-2026-06-07`. Raw: `/tmp/mma-diagnose-venue-ssh-results/*.json`.

## Verdict tally
SAFE-WITH-MUST-FIX ×4 (deepseek, nvidia, google, moonshot) · NOT-SAFE ×1 (qwen — same finding-set, stricter label). **Consensus: SAFE-WITH-MUST-FIX.**

## Consensus root causes (≥3/5), ranked
1. **`ListenAddress`=Tailscale-IP fails OPEN — 5/5 (unanimous).** Tailscale IPs are ephemeral (reboot / re-auth); `sshd` either fails to bind or falls back to `0.0.0.0:22`. With the firewall OFF that is LAN/WAN exposure. Root-cause: the security boundary depends on the runtime state of a *separate* service.
2. **Global firewall-OFF incompatible with exposing SSH — ≥4/5.** Cannot interface-scope `:22` while `advfirewall` is globally off; the two-trust-model is self-contradictory for `own` unless the firewall returns.
3. **Embedded key is non-revocable — 5/5 (unanimous).** `active/placeholder/revoked` is a label in the binary; compromise persists until rebuild+reflash; no revocation propagation to deployed pods.
4. **`VenueType` mutable config → backdoor-on-Sold / downgrade-strip — ≥4/5.** Misprovision sold-as-own = persistent admin backdoor on a third party (liability). Old installer re-run on an own pod silently strips SSH.
5. **Shadow unaudited admin channel — ≥4/5.** Interactive SSH bypasses rc-sentry/heart command-filter + audit (visibility regression).

## Consensus gaps (≥3/5)
- **Uninstall / Own→Sold downgrade cleanup** of `administrators_authorized_keys` + sshd disable — 5/5.
- `sshd -t` (syntax) before start + behavioral-verify must confirm `PasswordAuthentication no` via `sshd -T` — ≥3/5.
- ACL the **parent dir** `C:\ProgramData\ssh` (not just the file) — else file-replace via rename — ≥3/5; + idempotent ACL re-apply / drift check.
- **Service-start race:** Tailscale not initialized (no IP) before sshd starts → bind failure / 0.0.0.0 fallback — ≥3/5.
- Windows Update may reinstall OpenSSH.Server + reset config (nvidia). Audit-log key addition (nvidia/moonshot). Binary key-string extraction is fingerprintable (moonshot).

## Mitigation critique (consensus)
- **SUFFICIENT:** `default=Sold` (secure-by-default) · `PasswordAuthentication no` · behavioral-verify as a *baseline*.
- **WEAK → replace:** `ListenAddress`=Tailscale-IP (→ firewall-subnet rule) · placeholder-key-only (→ revocation / short-lived certs) · file-only ACL (→ parent-dir + idempotent re-check) · behavioral-verify-too-shallow (→ also assert password-auth disabled + only-provisioned-key succeeds).

## MUST-FIX folded into the PLAN (revised §5)
- **M1 (kills RC#1+#2): firewall ON for `own`, not off.** Do NOT inherit `advfirewall state off` for own venues. Instead: re-enable firewall + a single inbound allow-rule `:22` scoped to the **Tailscale subnet `100.64.0.0/10`** (and/or the Tailscale interface). This makes the boundary a *durable policy*, not a service-state dependency, and is fail-closed (if the rule/interface is absent, no inbound). Keep `ListenAddress` as defense-in-depth, never as the sole boundary.
- **M2 (kills RC#1 residue): sshd watchdog** — stop sshd if the Tailscale interface disappears (so a tailnet partition can't leave `:22` reachable on a re-enabled LAN path).
- **M3 (kills RC#4): immutable `VenueType` marker.** Default `Sold`; on first successful `own` install write a protected marker; refuse to flip own↔sold on re-run without explicit secure opt-in. Prevents backdoor-on-sold AND old-installer downgrade-strip (an own pod with the marker rejects the unconditional SSH-removal path).
- **M4 (kills RC#3): revocation path.** V1 pendrive: the embedded `trusted_ssh_keys` set drives an idempotent *reconcile* (add active keys, **remove** superseded/revoked keys from `authorized_keys`), so a re-provision rotates/revokes. V2 web-installer target: short-lived SSH certificates via `TrustedUserCAKeys` + online/cached revocation (documented as the migration target).
- **M5 (RC#5 + gaps): config-validate + behavioral-verify hardening.** `sshd -t` before start; after start assert via `sshd -T` that `passwordauthentication no` + only the provisioned key authenticates; enable Windows file-audit on `administrators_authorized_keys`; SSH-session command audit = documented debt (interactive ops still preferentially routed through audited rc-sentry/heart for fleet automation; SSH for interactive admin/debug/canary).
- **M6 (gaps): ACL parent dir + idempotency** (`icacls C:\ProgramData\ssh` inheritance:r grant 544/18; re-apply each run) · **service ordering** (sshd `depend= Tailscale`/delayed-auto + the M2 watchdog) · **uninstall/downgrade scrub** (Own→Sold removes keys + disables sshd).

**Disposition:** PROCEED to implement on `feat/installer-venue-ssh` with M1–M6 folded in. Foundational boundary → PR is **Captain-merge-gated** (no self-merge). Phase 1 (current 8 pods) remains gated on the operator-supplied rc-sentry key (separate path).
