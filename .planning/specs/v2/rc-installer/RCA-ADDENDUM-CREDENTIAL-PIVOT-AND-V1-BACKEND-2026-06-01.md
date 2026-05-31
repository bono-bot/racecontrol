# rc-installer V2 — §S-146 RCA ADDENDUM: credential pivot + V1-backend reuse (INC-4/4B/5)

**Date:** 2026-06-01 IST · **Branch:** TBD `feat/rc-installer-*` (trust core INC-1/2/3 currently uncommitted on `feat/heart-loading-complete-route`; will be re-branched)
**Author:** bono (rc-installer-core session) · **Extends:** `RCA-AND-DESIGN-V2-WEB-2026-05-31.md` (Part A §S-146 RCA + Part C MMA Step-1). This addendum does NOT supersede it; it adds the surfaces that RCA predates.
**Authority:** Captain this session — "unfreeze and pilot on BandlaGuda", "assign pod number from the installer", "remove the old pod programs", "token pre-integrated… Venue License Key is only for Server… make installation as easy as possible", and "Proceed".
**Status:** RCA addendum for the foundational gate. **No INC-4/4B/5 code lands until: (a) this addendum + (b) MMA Step-1 on the credential pivot + (c) per-PR Captain auth (§S-146) + (d) R-129 re-ratify for the `.in` host.** INC-1/2/3 (trust+transport, venue-agnostic, placeholder-keys-fail-closed) are NOT gated by this and are already built+green.

---

## Why an addendum (what the 2026-05-31 RCA did not cover)

| New surface (this session) | In the 2026-05-31 RCA? | Why it needs RCA coverage |
|---|---|---|
| **Credential pivot:** one-time install token → **durable Venue License Key**, **Server-only**; pods carry **no credential** | ❌ — that RCA assumed a per-install token | Touches **auth + venue-identity + DB-schema** = §S-146 foundational boundary |
| **Server-led enrollment** (server first → operator pod# → server auto-detects via mDNS + Register-upsert) | ❌ | Reuses the V1↔V2 **pod-state-channel** + WS-auth (`agent_auth`/`agent_register`) |
| **INC-4B old-pod cleaner** reversing the FULL V1 mutation surface (registry/PowerShell/tasks/services/netsh) | partial — RCA §1 noted hardening is "reusable", but not removal/reversal | Directly reuses + reverses V1 `main.rs` behaviour = V1-dependent |
| **`.app` → `.in`** host rename (`config.rs` already `console.racecontrol.in`) | ❌ — RCA says `.app` | R-129 locked-prefix literal change → re-ratify |
| **Pod number assigned at installer** (1–8) | ❌ — RCA §1 noted V1 `get_pod_number` but the prior plan draft had removed it | Reinstated as operator UX; opaque `pod_id` underneath |

---

## §1 — Boundary map (paths + lines) — credential + server-led surfaces

### 1a. Credential pivot boundary
| Surface | Location | V1/current → V2 pivot |
|---|---|---|
| Install credential | `rp-v2-apps/apps/racecontrol-console/lib/install-tokens.ts` (`mintInstallToken`/`redeemInstallToken`, single-use `UPDATE … WHERE consumed_at IS NULL`), `lib/db.ts` `install_tokens(token_hash, tenant_id, profile, …)` | one-time token (consumed, 410-on-reuse) → **durable Venue License Key**: re-installs/re-images succeed; **Server-scoped** |
| Installer client | `crates/rc-installer/src/{identity.rs, redeem_client.rs}` (INC-1/2, built) | wire shape `RedeemedIdentity` **unchanged**; only the **consume-once lifecycle relaxes** + scope=Server. Pod profile **does not call redeem at all** |
| Redeem route | `app/install/redeem/route.ts` (returns `{tenant_id,venue_id,profile,server_address,bono_address,racecontrol_toml}`, no WG secret) | accepts a durable key (or sibling `/install/activate`). **Replit-owned**; I own the installer-side contract |

### 1b. Server-led enrollment boundary (pod-state-channel — V1↔V2 shared)
| Surface | Location (file:line) | Disposition |
|---|---|---|
| Pod WS auth | `crates/racecontrol/src/ws/agent_auth.rs:81-100` — PSK (`config.cloud.terminal_secret`); **empty PSK allowed w/ backward-compat warning** → 24h JWT | REUSED as-is for the pilot (LAN-trust). Enrollment-window is the hardening (heart-side, deferred) |
| Pod register/upsert | `crates/racecontrol/src/ws/agent_register.rs:40-143` — `INSERT … ON CONFLICT DO UPDATE status='online', venue_id = state.config.venue.venue_id` | REUSED — the **server** stamps the pods-row venue_id. No installer change needed |
| mDNS | server `crates/racecontrol/src/mdns.rs` advertises `venue_id`/`build_id`; pod `crates/rc-agent/src/mdns_discovery.rs` browses + falls back to `[core].url` | REUSED — pod auto-detect already exists |
| rc-agent identity | `crates/rc-common/src/config_schema.rs:57-86` `PodConfig{number,…}` (pod# exists); **no `[venue]` section** | INC-5 ADDS serde-optional `[venue] venue_id` (COLD field) so live V1 binary tolerates the new toml (MMA G12) |

### 1c. V1 Windows backend reuse (INC-4) + reversal (INC-4B) — full mutation surface
The V1 `main.rs` mutation inventory INC-4 parameterizes and INC-4B detects/reverses (file:line, verified this session):
- **Processes** `main.rs:382-388` (rc-agent/rc-sentry/pod-agent/msedge/msedgewebview2)
- **Registry** Run keys `:728-766` (RCAgent/RCSentry set; PodAgent/RCWatchdog deleted); Edge policy `:703-718`; OpenSSH `:816-839`; `pod-lockdown.ps1` HKCU/HKLM kiosk + USBSTOR + WindowsUpdate keys
- **PowerShell** Defender `Add-MpPreference` `:310-330`; `Set-NetConnectionProfile Private` `:778-780`; `Remove-WindowsCapability` OpenSSH `:803-805`
- **Scheduled tasks** `install-watchdogs.cmd:9-10` (`RacingPoint\PodAgent`,`\RcAgent`)
- **Services** `sshd`/`salt-minion`/Hexnode set; `RCWatchdog`/`RaceControl`/`RCSentryServer` `install-*.bat`
- **netsh** persistent `excludedportrange` 8090/8091/18923-25 `:366-377`; firewall-off `:783`; portproxy 80→8080
- **Dirs** `C:\RacingPoint\`, `%LOCALAPPDATA%\RacingPoint\EdgeProfile\`
- **Key finding:** V1 on reinstall ONLY overwrites binaries — registry policies, Defender exclusions, persistent port reservations, tasks, services, dirs are **never reversed** → INC-4B is net-new reversal, not a copy of V1.

## §2 — Inherited-issue catalogue (beyond the 2026-05-31 list)
- **I-7 single-use credential breaks re-install/re-image** — a baked single-use token 410s forever on any re-run (the exact reason for the durable-key pivot). *(This is why "bake the token into each package" is rejected — credential-less pods + a durable Server key is strictly better.)*
- **I-8 credential-in-signed-binary breaks byte-identical-binary invariant** — personalizing the signed `.exe` per venue defeats one-artifact trust. Pivot keeps the binary generic; ease comes from pods needing nothing.
- **I-9 LAN-trust admission** — empty-PSK-allowed means any LAN device can self-declare a pod#. **MMA Step-1 rejected "open empty-PSK is fine for the pilot" (Q-PILOT = NO, §7).** Captain decision 2026-06-01: **VLAN isolation** — Server+pods on a dedicated switch/VLAN with no customer-WiFi uplink, documented as a **hard pilot prerequisite**. Keeps zero-typing pods + adds no code; the heart-side enrollment-window remains the fix for venues that cannot guarantee isolation.
- **I-10 cleaner over-reach** — reversing kiosk lockdown during a reinstall would unlock a live pod. Mitigated by the **two-mode** split: default `clean-for-reinstall` keeps posture; only `decommission` reverses it.
- **I-11 cleaner destroys live data** — removing services/killing procs on a pod mid-session corrupts billing (MMA G1). Mitigated by the drain gate (INC-9 runbook) + detect→confirm→verify.

## §3 — Past-bug disposition
| Issue | Disposition |
|---|---|
| I-7 single-use re-install | **root-caused by pivot** — durable Venue License Key |
| I-8 per-package credential | **root-caused** — binary stays generic+signed; pods credential-less |
| I-9 LAN-trust | ⚠️ **CORRECTED BY MMA (§7) → Captain-decided 2026-06-01: VLAN ISOLATION.** MMA rejected deferral for a LIVE billing venue (Q-PILOT=NO). Resolution = Server+pods on a dedicated VLAN/switch, no customer-WiFi uplink, as a **documented hard pilot prerequisite** (zero-typing pods preserved, no new code). Heart-side enrollment-window stays the fix for non-isolatable venues. |
| I-10 cleaner over-reach | **designed-out** — default mode preserves kiosk posture |
| I-11 cleaner data-loss | **MMA upgraded to MUST-BLOCK** — drain gate must be a CODE guard in the cleaner (query session status → abort on active), NOT a runbook line; detect/confirm/verify + clone-rehearsal (MMA G14) before any live pod |
| V1 hardening reuse (RCA §1) | **root-caused** — INC-4 parameterizes behind `WindowsBackend` trait; V1 `main.rs` keeps compiling against the trait with legacy constants (reversible) |

## §4 — V2-alignment delta
The pivot makes V2 **more** V2-aligned, not less: credentials root in the Server (a durable venue identity) instead of per-install tokens; pods become zero-config consumers admitted by the existing pod-state-channel; the binary stays byte-identical (the trust-core invariant). INC-4 keeps V1's hard-won Windows mechanics but inverts ownership of the constants (parameterized, not baked). INC-4B is the missing reversal half V1 never had.

## §5 — V2-framed proposal (gated)
1. **MMA Step-1 DIAGNOSE** on the credential pivot (durable-key lifecycle, credential-less-pod admission, cleaner reversal safety) — OpenRouter, 5 models / ≥3 families, per doctrine. **Required before INC-5 code.**
2. **Mechanism-trust-check** (the `~/.claude/CLAUDE.md` 5-question check) on the **pod-state-channel** before the INC-4/4B fix, since INC-4B reuses delivery/supervision surfaces (kill+swap of rc-agent). Cache to `racecontrol/.planning/specs/v2/MECHANISM-TRUST/`.
3. **Per-PR Captain auth (§S-146)** for the credential pivot PR (auth/venue-identity/DB-schema = foundational; standing-autonomy verbs do NOT satisfy).
4. **R-129 re-ratify** for the `.in` host literal (`INSTALLER_TIER_1_BASE`/`INSTALLER_DOWNLOAD_URL_PREFIX` + golden vectors + `manifest_signature.rs` fixture all already `.in` in the working tree).
5. Build order unchanged: INC-4 (parameterize) → INC-4B (cleaner) → INC-5 (provisioner + `[venue]` schema). The credential pivot lands with INC-5/INC-7 (route side = Replit).

## §6 — Open items carried to MMA Step-1
- Durable key storage at the Server (where; rotation; revoke).
- Credential-less pod admission blast radius vs the enrollment-window timing.
- Cleaner: is "the known V1 set" complete on a real BandlaGuda image? (clone-rehearsal, MMA G14.)
- `/install/redeem` durable vs sibling `/install/activate` (Replit contract).

## §7 — MMA Step-1 DIAGNOSE RESULTS (2026-06-01)

**Surface `MMA-RC-INSTALLER-CRED-PIVOT-bono-2026-06-01` · 5 models / 5 vendor families** (deepseek-r1-0528, claude-sonnet-4.6, gpt-5.4, gemini-2.5-pro, kimi-k2.5) · **5/5 OK** · **$0.22644** · raw `/tmp/mma-cred-pivot-out.txt` · spend → `comms-link/data/openrouter-spend-bono.jsonl`. **Coverage caveat (honest):** 2 of 5 (deepseek, sonnet) hit `max_tokens` and truncated before their final `must_block`/verdict block — their findings are partially captured; gpt/gemini/kimi returned complete JSON.

### Q-PILOT — is deferring the enrollment window past the LIVE pilot acceptable?
**Verdict: NO (unanimous among those that reached it).** 3/3 models that returned the verdict (gpt, gemini, kimi) said **no**; the other 2 (deepseek, sonnet) truncated before the verdict but both flagged empty-PSK-on-live-LAN as CRITICAL in their findings. **None said yes/conditional.** → **This corrects this addendum's original I-9 "defer is fine for pilot" disposition AND bono's earlier reply to the UI session (Q3).**

The acceptable-mitigation set (you need ONE before the live pilot, not necessarily the full heart-side window):
- **(a)** the time-boxed enrollment window (operator opens it on the Server), OR
- **(b)** a per-install PSK for the pilot pods (small typing, removed once (a) ships), OR
- **(c)** physical LAN isolation (Server+Pods on a dedicated VLAN/switch with no customer-WiFi uplink) as a **documented hard pilot prerequisite**. (c) preserves the zero-typing pod UX. **← Captain chose (c) on 2026-06-01.**

### Consensus findings (enumerated across all 5 raw outputs)
| # | Finding | Models | Severity |
|---|---|---|---|
| F1 | Credential-less pods / empty-PSK LAN trust → pod impersonation + billing fraud at a live venue | **5/5** | CRITICAL |
| F2 | Server stamps pods-row `venue_id` from its OWN config → wrong venue_id = cross-venue billing contamination; needs activation-time venue_id match-check + operator confirm | **5/5** | CRITICAL/HIGH |
| F3 | Cleaner "known V1 set" likely incomplete on the real BandlaGuda image → clone-rehearsal (G14) mandatory before live | **5/5** | HIGH |
| F4 | Durable key: no revocation/rotation path → leaked key = permanent venue takeover | **4/5** (deepseek, sonnet, gpt, gemini) | CRITICAL |
| F5 | Durable key: no machine/hardware binding → multi-server split-brain / off-site replay | **4/5** (sonnet, gpt, gemini, kimi) | CRITICAL |
| F6 | Cleaner destroys live billing state if run on an active pod → drain gate must be a CODE guard, not a runbook | **4/5** (deepseek, sonnet, gpt, kimi) | CRITICAL |
| F7 | DB-schema: single-use→durable semantic break → use a NEW `venue_license_keys` table + migration, don't overload `install_tokens` | **4/5** (sonnet, gpt, gemini, kimi) | HIGH |
| F8 | Cloud aggregator `default_venue_id` coupling amplifies a single misconfig cross-venue; remove the default fallback / fail-closed-until-activated | **4/5** (deepseek, sonnet, gpt, kimi) | HIGH/MEDIUM |
| F9 | Durable key plaintext storage → use OS cred store (DPAPI/TPM) not toml | **3/5** (sonnet, gpt, kimi) | HIGH |
| F10 | Pod-number collision: unconditional UPSERT silently displaces a live pod mid-session | **3/5** (sonnet, gpt, gemini) | CRITICAL |
| F11 | Cleaner reversal ordering/idempotency/resume (delete Run keys+tasks BEFORE killing procs; resumable) | **3/5** (sonnet, gpt, gemini) | HIGH |
| F12 | Cleaner mode boundary (clean vs decommission) needs a runtime guard + typed confirm, not just operator selection | **3/5** (sonnet, gpt, kimi) | HIGH |
| F13 | Cleaner verification dishonesty: verify by reading post-STATE, not command-success; add an audit scan for V1-pattern residue outside the known set | **2/5** (sonnet, gpt) | MEDIUM |
| F14 | 24h JWT after empty-PSK extends a momentary LAN race to day-long impersonation → short provisional token in pilot mode | 1/5 (gpt) | HIGH |
| F15 | mDNS fallback can attach a pod to the wrong Server with no venue check (already mitigated in plan: pod asserts mDNS TXT venue_id == token venue_id) | 1/5 (gpt) | HIGH |
| F16 | rc-agent `[venue]` field: if V1 PodConfig uses `deny_unknown_fields`, the new toml section breaks V1 parse → grep V1 + stage-test before writing it | 1/5 (sonnet) | MEDIUM |
| F17 | `.app→.in` rename: any env still pointing at `.app` silently fails manifest fetch → DNS/CNAME sunset + firewall audit | 1/5 (sonnet) | MEDIUM |

### MUST-BLOCK (consolidated — gate the credential pivot PR; from the models' `must_block` lists)
1. **Pod admission** — no open empty-PSK at a live venue: ship (a)/(b)/(c) above before the BandlaGuda pilot. *(F1; gpt+gemini+kimi)*
2. **Durable key revocation + single-active-server** — revoke/rotate path + reject 2nd concurrent activation. *(F4/F5; sonnet+gpt+gemini)*
3. **Cleaner drain gate IN CODE** — query session status, abort on active session; not a runbook. *(F6/I-11; sonnet+gpt+kimi)*
4. **venue_id activation match-check** — Server refuses to admit pods unless config venue_id == redeemed-key venue_id; remove default fallback. *(F2/F8; gpt+gemini)*
5. **DB-schema redesign + reviewed migration** — new `venue_license_keys` table, not `install_tokens` overload. *(F7; sonnet+gpt)*

### Disposition for the build
- F6/F10/F11/F12/F13/F16/F17 are **installer/cleaner-side (mine)** → fold as hard requirements into INC-4B (drain-gate code, ordering, two-mode guard, state-not-echo verify, audit scan) + INC-5 (F16 V1 serde grep) + R-129 (F17).
- F1/F2/F4/F5/F7/F8/F10(UPSERT)/F14 are **heart + console-side (racecontrol heart session + Replit)** → must be coordinated; the credential pivot PR cannot land installer-side alone.
- The pilot pod-admission choice (a/b/c) is **Captain-stake** — surfaced for decision.
