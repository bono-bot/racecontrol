# Anti-Cheat Compatibility Matrix
**Created:** 2026-03-21
**Version:** v15.0 Phase 107
**Purpose:** Per-game reference for what rc-agent subsystems are safe to run during each game session
**Audience:** Staff, developers, and ops team
**Cross-reference:** See [risk-inventory.md](risk-inventory.md) for full behavior audit with source file:line references

---

## Game Anti-Cheat System Reference

| Game | Anti-Cheat System | Kernel Level | Active When | Confidence |
|------|------------------|--------------|-------------|------------|
| F1 25 | EA AntiCheat (EAAC / Javelin) | YES | Game running | MEDIUM |
| iRacing | Epic Online Services (EOS) | YES | Game running | MEDIUM-HIGH |
| LMU (Le Mans Ultimate) | Easy Anti-Cheat (EAC) | YES | Multiplayer sessions | MEDIUM |
| AC EVO | Unknown (Early Access v0.5.4) | UNKNOWN | Unknown | LOW |
| EA WRC | EA AntiCheat (EAAC / Javelin) | YES | Game running | MEDIUM |
| Assetto Corsa (original) | None | NO | N/A | HIGH |

> **Note:** F1 25 uses EA Javelin (EAAC), NOT EAC. iRacing migrated from EAC to Epic EOS in May 2024. These distinctions matter for detection behavior.

---

## Per-Game Compatibility Matrix

| rc-agent Subsystem | F1 25 (EAAC) | iRacing (EOS) | LMU (EAC) | AC EVO (Unknown) | EA WRC (EAAC) | AC Original (None) |
|-------------------|--------------|---------------|-----------|------------------|--------------|---------------------|
| Keyboard hook (SetWindowsHookEx WH_KEYBOARD_LL) | UNSAFE -- EAAC scans hook chain | UNSAFE -- EOS scans hook chain | UNSAFE -- EAC scans hook chain | SUSPEND -- treat as protected | UNSAFE -- EAAC scans hook chain | SAFE -- no AC |
| Process guard (continuous enumeration + kill) | SUSPEND -- EAAC monitors open handles | SUSPEND -- EOS monitors process access | SUSPEND -- EAC monitors open handles | SUSPEND -- treat as protected | SUSPEND -- EAAC monitors open handles | SAFE -- no AC |
| Process guard (one-time startup snapshot) | SAFE -- snapshot before game launch | SAFE -- snapshot before game launch | SAFE -- snapshot before game launch | SAFE -- before game | SAFE -- snapshot before game launch | SAFE |
| iRacing SDK shared memory read | N/A | SAFE -- iRacing staff confirmed explicitly | N/A | N/A | N/A | N/A |
| LMU rF2 shared memory read | N/A | N/A | SAFE -- read-only, same model as iRacing SDK | N/A | N/A | N/A |
| AC shared memory read (physics/graphics/static) | N/A | N/A | N/A | GATE -- feature-flagged OFF until v1.0 | N/A | SAFE -- no AC |
| F1 25 UDP telemetry (port 20777) | SAFE -- UDP, no process access | N/A | N/A | N/A | N/A | N/A |
| EA WRC UDP telemetry | N/A | N/A | N/A | N/A | SAFE -- UDP, no process access | N/A |
| Health HTTP endpoint (:8090) | SAFE -- localhost TCP | SAFE -- localhost TCP | SAFE -- localhost TCP | SAFE -- localhost TCP | SAFE -- localhost TCP | SAFE |
| WebSocket to racecontrol | SAFE -- TCP, no game contact | SAFE -- TCP, no game contact | SAFE -- TCP, no game contact | SAFE -- TCP, no game contact | SAFE -- TCP, no game contact | SAFE |
| Billing lifecycle | SAFE -- independent of game process | SAFE -- independent | SAFE -- independent | SAFE -- independent | SAFE -- independent | SAFE |
| Lock screen (PIN auth) | SAFE -- UI only | SAFE -- UI only | SAFE -- UI only | SAFE -- UI only | SAFE -- UI only | SAFE |
| Overlay window | GATE -- LOW risk, separate window, not injection | GATE -- LOW risk | GATE -- LOW risk | GATE -- treat as protected | GATE -- LOW risk | SAFE |
| Ollama LLM queries | SUSPEND -- GPU/VRAM contention visible to EAAC | SUSPEND -- GPU contention | SUSPEND -- GPU contention | SUSPEND -- treat as protected | SUSPEND -- GPU contention | SAFE |
| Registry writes (HKLM/HKCU) | GATE -- defer during session | GATE -- defer during session | GATE -- defer during session | GATE -- defer during session | GATE -- defer during session | SAFE |
| ConspitLink (wheelbase software) | [See ConspitLink Audit] | [See ConspitLink Audit] | [See ConspitLink Audit] | [See ConspitLink Audit] | [See ConspitLink Audit] | SAFE |
| Unsigned rc-agent.exe binary | MEDIUM risk -- composite signal | MEDIUM risk -- composite signal | MEDIUM risk -- composite signal | LOW risk | MEDIUM risk -- composite signal | SAFE |

Each subsystem row above corresponds to behaviors documented in detail in [risk-inventory.md](risk-inventory.md) with exact source file:line references.

---

## Legend

| Status | Meaning | Phase to Address |
|--------|---------|------------------|
| SAFE | No anti-cheat risk. Continue normally. | None |
| UNSAFE | Must be REMOVED before any protected game runs. | Phase 108 |
| SUSPEND | Must be DISABLED during protected game session. Resume after game exit + 30s cooldown. | Phase 109 |
| GATE | Must be DELAYED or CONDITIONAL during game session. | Phase 109/110 |
| N/A | Subsystem not relevant for this game. | None |

---

## Key Takeaways

1. **SetWindowsHookEx is UNSAFE for ALL kernel-level AC games.** Phase 108 must replace it with GPO registry keys before any canary testing.
2. **Process guard must be SUSPENDED during ALL protected game sessions.** Phase 109 safe mode gates this.
3. **Telemetry reads are SAFE for all current games** (iRacing SDK, LMU shared mem, F1 25 UDP) except AC EVO which is feature-flagged off.
4. **Ollama queries must be SUSPENDED** during protected game sessions due to GPU/VRAM contention.
5. **Code signing reduces composite risk** but is not a hard ban trigger alone. Phase 111.
6. **ConspitLink verdict pending audit** -- see docs/anticheat/conspit-link-audit.md.
7. **Full behavior details** with source file:line references are in [risk-inventory.md](risk-inventory.md).

---

## TOML Safe Mode Config Preview

```toml
# Preview -- to be implemented in Phase 109
[safe_mode]
cooldown_secs = 30

[safe_mode.subsystems]
keyboard_hook = "removed"       # Phase 108 -- permanently replaced by GPO
process_guard = "suspend"       # Disable all kill operations
ollama_queries = "suspend"      # Disable GPU/VRAM contention
registry_writes = "defer"       # Queue until safe mode exits
overlay = "gate"                # Allow but monitor
telemetry_shm = "gate"          # Per-game config in [games.*]

[games.f1_25]
anti_cheat = "eaac"
telemetry_type = "udp"
safe_mode_required = true

[games.iracing]
anti_cheat = "eos"
telemetry_type = "shm"
shm_safe = true                 # iRacing staff confirmed
safe_mode_required = true

[games.lmu]
anti_cheat = "eac"
telemetry_type = "shm"
shm_safe = true                 # Read-only, official plugin API
safe_mode_required = true

[games.ac_evo]
anti_cheat = "unknown"
telemetry_type = "shm"
shm_safe = false                # Feature-flagged off until v1.0
safe_mode_required = true

[games.ea_wrc]
anti_cheat = "eaac"
telemetry_type = "udp"
safe_mode_required = true

[games.assetto_corsa]
anti_cheat = "none"
telemetry_type = "shm"
shm_safe = true
safe_mode_required = false
```
