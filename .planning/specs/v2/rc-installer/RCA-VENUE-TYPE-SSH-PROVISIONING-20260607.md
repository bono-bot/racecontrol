# §S-146 RCA — Venue-type-aware SSH provisioning in rc-installer (2026-06-07)

**Author:** bono (sole pilot, §S-448) · **Branch:** `feat/installer-venue-ssh` (off `origin/main` `983ca527`)
**Trigger:** Captain direction — *"make it easy for all Racing Point eSports venues to touch the pods … directly SSH into each pod … via RaceControl. For all other venues that do not belong to Racing Point Esports, they will need the installer file."* (this session, 2026-06-07)
**Boundary class:** FOUNDATIONAL — pod **auth/access** boundary. → per `racecontrol/CLAUDE.md` "V1-dependent V2 sections" + harness `~/.claude/CLAUDE.md`, escalates to **MMA Step 1 DIAGNOSE before PLAN** + **per-PR Captain merge auth**. This RCA is the precondition; it does **not** authorize merge.
**Scope note:** This RCA covers **Phase 2** (installer becomes venue-type-aware — going-forward capability). It does **not** cover **Phase 1** (bootstrapping the 8 *already-installed* VLM pods over rc-sentry `:8091/exec`), which is blocked on the operator-supplied rc-sentry key and is a separate, audited, runtime path.

---

## 1. Boundary map (paths + lines)

**rc-installer V1 pendrive bin** (`crates/rc-installer/src/main.rs`; `#[cfg(not(windows))] compile_error!` — Windows-only, pure-std, does NOT use the trust-core lib):
- `run_installation()` 14-step sequence: **step 12 "Removing legacy programs"** (220-221) → `remove_legacy_programs()` (774-970).
- **SSH teardown (the surface this RCA forks):**
  - sshd service stop + delete + taskkill (785-795)
  - `OpenSSH.Server` capability `Remove-WindowsCapability` (797-813)
  - `reg delete HKLM\SOFTWARE\OpenSSH` (815-822)
  - `OpenSSHD` Run-key delete (824-839)
  - **`fs::remove_file(C:\ProgramData\ssh\administrators_authorized_keys)`** + `sshd-loop.bat` (841-853)
- **Firewall disabled fleet-wide:** `netsh advfirewall set allprofiles state off` (783).
- Network set Private (778-780). Pod identity from CLI arg (`get_pod_number()` 1422-1424); `--yes`/`-y` auto-confirm (98). Process helpers `run()` (1327), `run_ps()` (1336).
- Doc comment 771-773: *"Tailscale is the replacement for remote access -- never touch it."* ← the single-trust-model assumption this change splits.

**rc-installer trust-core lib (V2 web-installer):**
- `profile.rs` — `Profile {Server, Pod}`, deliberately minimal; `tests/d_installer_1.rs` **forbids** `online/offline/flavor/stub/isolated` in the label. → venue ownership MUST be a separate axis, not a `Profile` variant.
- `trusted_keys.rs` — embedded ed25519 `TrustedKeySet` with `status ∈ {active, placeholder, revoked}`, fail-closed lookup. → the template for an embedded **SSH** key set.
- `signature_verifier.rs`, `manifest.rs`, `config.rs`.

**Pod auth boundary (foundational):**
- Windows OpenSSH admin auth = `C:\ProgramData\ssh\administrators_authorized_keys` (NOT `~/.ssh`); sshd **silently ignores** the file unless ACL = Administrators + SYSTEM only.
- rc-sentry `:8091/exec` service-key channel (separate; **retained** as audited automated channel + fallback — `crates/rc-sentry/src/main.rs`).
- Heart `POST /pods/{id}/exec` WS-proxy with SEC-P0-10 command filter + audit + WhatsApp alert (`crates/racecontrol/src/api/pod_exec.rs`).

**Identities / topology:** control_node SSH pubkey `/root/.ssh/id_ed25519.pub` (`ssh-ed25519 …NXHsKePP… bono@racingpoint.in`). Tailnet `sim1-1`..`sim8` (rp-vlm binding of `pod` role per `VENUE-NODE-ROLE-TAXONOMY.md`); LAN `192.168.31.x` not control-node-routable.

---

## 2. Inherited-issue catalogue (V1 failure modes touching this boundary)

| ID | V1 failure mode | Source |
|---|---|---|
| IH-1 | **Inline PowerShell over SSH wiped all 8 pod TOMLs** (fleet down 25 min, 2026-04-08) — `Set-Content`/`$_` mangled by Git-Bash expansion. | racecontrol/CLAUDE.md "NEVER edit remote files via inline PowerShell over SSH" |
| IH-2 | SSH banner / pipe corruption prepended garbage to `racecontrol.toml`; TOML parse failed from line 1, silent empty-defaults for 2h (2026-03-24). | CLAUDE.md "Never pipe SSH output into config files" |
| IH-3 | `administrators_authorized_keys` **ACL pitfall** — sshd silently ignores a world-readable key file; key login fails with no error. | Windows OpenSSH behavior; this session's bootstrap design |
| IH-4 | Installer turns the **Windows firewall fully off** (`advfirewall state off`, 783) — pods rely on "behind LAN router"; opening `:22` under firewall-off = no interface restriction. | main.rs:783 |
| IH-5 | V1 remote-access **mess** — `sshd-loop.bat`, salt-minion, pod-agent, Hexnode MDM were crude/insecure remote-mgmt; the wholesale removal was correct *for that mess*. | main.rs 841-967 |
| IH-6 | **Unauthorized-persistence / key-provenance** — installing an authorized key is a persistence action; a self-discovered or branch-extracted key must never be trusted. | harness classifier denials this session; `feedback_replit_ssh_stable_key_in_secret_not_ephemeral` |
| IH-7 | rc-sentry exec **self-kill / BLOCKED_PATTERNS / EPERM-as-success** (PR #66 class, 7 pods burned). | CLAUDE.md mechanism-trust-check; rc-sentry main.rs |
| IH-8 | `.spawn().is_ok()` / `schtasks` returns success but service never started (silent non-launch). | CLAUDE.md "`.spawn().is_ok()` does NOT mean the child started" |

---

## 3. Past-bug disposition

- **IH-1** → ROOT-CAUSED-AND-FIXED (doctrine: write-local→scp). **Applies, but blast radius is lower:** `provision_ssh()` runs **on the pod at install time** via `std::process` (local), not inline-PS *from the control node over SSH*. The only PS is local on the pod. Mitigation: any sshd_config edit uses a scp'd/local `.ps1` or native API on the pod — never control-node→pod inline PS.
- **IH-2** → ROOT-CAUSED-AND-FIXED. NOT-APPLICABLE to install-time provisioning (no remote `cat` into config).
- **IH-3** → KNOWN-MUST-DESIGN-IN: `icacls … /inheritance:r /grant *S-1-5-32-544:F /grant *S-1-5-18:F` (SIDs for locale independence); **verify by an actual key login, not file presence** (composes with IH-8).
- **IH-4** → UNRESOLVED-STRUCTURAL (the installer's wholesale firewall-off predates this work). Phase 2 does NOT fix the firewall posture globally; it **scopes the new exposure**: bind sshd to the **Tailscale interface** (`ListenAddress <tailscale-ip>`) and/or an inbound `:22` rule limited to the Tailscale interface, so re-enabling SSH does not widen LAN attack surface. The global firewall-off is logged as **carry-forward security-debt** (`comms-link/data/security-debt-ledger.jsonl`, class=policy-gap, closure-trigger: installer firewall redesign).
- **IH-5** → ROOT-CAUSED. Legacy removal **stays** for `sold`; `own` gets a **clean key-only sshd**, never the `sshd-loop.bat` pattern.
- **IH-6** → DESIGN-IN: embed only a **Captain-verified** control_node pubkey, mirroring `trusted_keys.rs` `status` model (`placeholder` pre-ceremony → `active` after Captain states the fingerprint out-of-band). Never embed a key whose material came from a non-Captain-verified source.
- **IH-7** → NOT-APPLICABLE to Phase 2 (installer provisions at install; no rc-sentry exec chicken-and-egg). Applies to **Phase 1** (separate).
- **IH-8** → DESIGN-IN: after `sc start sshd`, verify the service is `RUNNING` (poll) **and** behavior (a key login) before reporting success; `.is_ok()` is insufficient.

---

## 4. V2-alignment delta

**Current (V1 inertia):** ONE trust model — SSH is pure attack surface → always removed; firewall off; Tailscale is the sole remote-access path. No venue-ownership concept.

**V2 target:** TWO trust models keyed on **venue ownership**:

| Venue type | Interactive transport | Automated / fallback | Installer behavior |
|---|---|---|---|
| **own** / RP-Esports (has Tailscale) | key-only SSH over Tailscale (control_node key, ACL-correct, interface-bound) | rc-sentry `:8091/exec` + heart `/pods/{id}/exec` (both retained, audited) | **provision** OpenSSH + key |
| **sold** / third-party (no Tailscale) | none (installer file) | rc-sentry `:8091/exec` | **remove** OpenSSH (today's hardening) |

Selected by a new `VenueType {Own, Sold}` axis (orthogonal to `Profile {Server, Pod}`), addressed by role per `VENUE-NODE-ROLE-TAXONOMY.md`. Composes with the **"Open-by-default, flagged-to-close"** security-debt model for the firewall carry-forward (IH-4).

**The gap:** the installer encodes "SSH = legacy, always remove" as an invariant. V2 needs that to be a **venue-type-conditional** decision, with the `own` path adding a *correctly-hardened* sshd rather than reintroducing the V1 sshd-loop mess.

---

## 5. Proposed change (V2-framed)

**V2 doctrine alignment:** moves the pod access boundary toward the venue-ownership two-trust-model (Captain 2026-06-07 direction) + `VENUE-NODE-ROLE-TAXONOMY.md` role addressing + "Open-by-default flagged-to-close" (IH-4 debt). Retains rc-sentry/heart audited channels (complement, not replace).

1. **New axis `VenueType {Own, Sold}`** — own module (e.g. `venue_type.rs`); **default `Sold`** (= current safe behavior; opening SSH must be an explicit opt-in). Source: V1 bin CLI `--venue-type own|sold`; V2 web-installer = a manifest/profile-picker field. NOT a `Profile` variant (respects `d_installer_1.rs`).
2. **Fork step 12:** `Sold` → `remove_legacy_programs()` unchanged (incl. SSH removal). `Own` → `remove_legacy_programs_except_ssh()` (legacy/MDM/salt/pod-agent removal retained; SSH block skipped) **then** `provision_ssh()`.
3. **`provision_ssh()`** (all on-pod, local `std::process` / native): install `OpenSSH.Server` capability + `sc config sshd start= auto`; write **embedded control_node pubkey** to `administrators_authorized_keys` (idempotent find-then-append); `icacls /inheritance:r /grant *S-1-5-32-544:F /grant *S-1-5-18:F` (IH-3); sshd_config `PasswordAuthentication no` + `PubkeyAuthentication yes`; **`ListenAddress` bound to the Tailscale interface** + inbound `:22` rule scoped to that interface (IH-4); `sc start sshd`; **verify RUNNING + behavioral (key login) before success** (IH-8).
4. **Embedded SSH key set** — `trusted_ssh_keys.rs` mirroring `trusted_keys.rs` (`status` active/placeholder/revoked, fail-closed). Pre-ceremony = `placeholder` (IH-6).
5. **Carry-forward debt** — append `security-debt-ledger.jsonl` entry for the wholesale firewall-off (IH-4) with closure-trigger.
6. **Mirror** own/sold + SSH-provision into the V2 web-installer design (`.planning/specs/v2/rc-installer/RCA-AND-DESIGN-V2-WEB-2026-05-31.md` profile picker).

**Tests (Linux-buildable, trust-core lib):** `VenueType` parse/default/round-trip; step-12 dispatch selects provision-vs-remove by `VenueType` (logic extracted to a Linux-testable fn); golden-vector for the embedded-key idempotent-append decision. Windows-only `provision_ssh()` body = wine boot-test + a real pod canary (Phase 1 key-gated) for behavioral verify.

**Gate sequence (remaining):** this RCA → **MMA Step 1 DIAGNOSE** (foundational auth boundary) → PLAN → implement on `feat/installer-venue-ssh` → **PR (Captain merges; never self-merge)**. Phase 1 (current 8 pods) remains gated on the operator-supplied rc-sentry key.
