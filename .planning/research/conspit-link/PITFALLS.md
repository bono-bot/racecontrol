# Domain Pitfalls

**Domain:** Sim racing wheelbase fleet management (Conspit Ares 8Nm / OpenFFBoard / Conspit Link 2.0)
**Researched:** 2026-03-20
**Confidence:** MEDIUM-HIGH (verified against OpenFFBoard docs, DirectInput specs, and venue operator experience reports)

## Critical Pitfalls

Mistakes that cause safety incidents, fleet downtime, or force a full rearchitecture.

---

### Pitfall 1: Orphaned DirectInput FFB Effects on Game Process Death

**What goes wrong:** When a game process is force-killed (taskkill, crash, rc-agent session end), the game never calls `IDirectInputEffect::Unload()` or `IDirectInputDevice8::Unacquire()`. The last FFB effect (often a constant force or spring at high magnitude) remains active in the firmware. The wheel jerks to one side and stays locked there. This is the exact stuck-rotation bug documented in PROJECT.md.

**Why it happens:** DirectInput is a cooperative protocol -- the game "owns" effects it downloads to the device. When the game dies without cleanup, those effects persist in the OpenFFBoard firmware's effect engine. The firmware has no timeout; it holds the last commanded force vector indefinitely. ConspitLink 2.0 sits as middleware but does NOT clear orphaned effects when it detects the game process has exited.

**Consequences:**
- Wheel locked at arbitrary angle with up to 8Nm of torque (enough to injure fingers/wrists)
- Staff must physically intervene every session -- defeats the entire automation goal
- Power cycling unreliable because firmware may restore last effect state from EPROM

**Warning signs:**
- Wheel visibly rotates the instant a game window closes
- `ffb_controller.rs` ESTOP (0x0A) fires but wheel does not respond (P-20: ConspitLink overwrites)
- Inconsistent behavior between games (some clean up better than others)

**Prevention:**
1. Do NOT rely on the game to clean up. Assume every game exit is a crash.
2. After detecting game process exit, send OpenFFBoard commands in this specific order:
   - `fxm.reset` (clear all downloaded effects from the effect engine)
   - `axis.idlespring` set to a safe centering value (e.g., 30-50% -- smooth return, not a snap)
   - Only THEN set `fxm.ffbstate` to 0 if you want full disable
3. The ESTOP (`main.estop`) is a nuclear option -- it cuts ALL motor output including your centering spring. Use it only for true emergencies (customer panic), not routine session transitions.
4. Time the cleanup: poll for game process death, then wait 200-500ms (let ConspitLink finish its own reaction), then send your commands. This avoids the P-20 race.

**Phase:** Phase 1 (the stuck-rotation fix). This is the single most important pitfall to solve first because it blocks every other feature and is a safety issue.

**Confidence:** HIGH -- based on OpenFFBoard command documentation, DirectInput API behavior, and the observed symptoms in PROJECT.md.

---

### Pitfall 2: ConspitLink Overwrites HID Commands (P-20)

**What goes wrong:** rc-agent sends a zero-force or ESTOP HID command directly to the wheelbase, but ConspitLink 2.0 is also running and periodically writes its own FFB state to the device. ConspitLink wins the race because it runs a polling loop that reasserts its force profile. Your carefully crafted HID zero gets overwritten within milliseconds.

**Why it happens:** ConspitLink 2.0 acts as the "owner" of the device. It maintains its own internal FFB state model and pushes it to hardware on a timer. When rc-agent writes directly to the HID endpoint, ConspitLink does not know about it and simply overwrites on its next cycle.

**Consequences:**
- ESTOP appears to work for a split second then wheel re-engages
- Intermittent "it works sometimes" behavior that is maddening to debug
- False confidence that safety is handled when it is not

**Warning signs:**
- HID write succeeds (no error) but wheel behavior does not change
- Wheel briefly goes limp then snaps back
- Behavior differs depending on timing of the command

**Prevention:**
1. Do NOT fight ConspitLink for HID access during normal operation. Instead, work WITH it:
   - Use ConspitLink's own preset/profile system for FFB changes
   - Load a "safe centering" preset via ConspitLink's config files rather than raw HID
2. Reserve direct HID commands (ESTOP) for genuine emergencies where ConspitLink may be unresponsive
3. If you must send raw HID, first STOP ConspitLink's FFB loop (either by loading a zero-force preset or by toggling its internal state via its JSON config), wait for it to take effect, then send your HID command
4. Consider a sequencing protocol: rc-agent signals intent -> ConspitLink applies safe state -> rc-agent verifies via HID read

**Phase:** Phase 1 (must be solved together with the stuck-rotation bug -- they are the same problem viewed from different angles).

**Confidence:** HIGH -- P-20 is documented and observed in production.

---

### Pitfall 3: Safety -- Wheel Snap-Back Torque and Customer Injury

**What goes wrong:** During session transitions, FFB preset loads, or firmware glitches, the wheel suddenly applies full 8Nm torque in one direction. A customer whose hands are on the wheel (or reaching for the wheel) suffers a wrist sprain, jammed fingers, or worse.

**Why it happens:**
- A spring-centering force applied with no ramp-up snaps the wheel to center at full torque
- Loading a new preset that has high constant-force while the wheel is off-center
- Game launch applies FFB before the customer is ready
- ESTOP release re-enables last active force profile (which may be non-zero)

**Consequences:**
- Customer injury (documented in sim racing community: broken thumbs, dislocated fingers, wrist sprains from direct drive wheels)
- Liability for the venue even with waivers
- Trust damage -- customers become afraid of the equipment

**Warning signs:**
- Audible "thunk" from the wheelbase during transitions
- Customer flinching or pulling hands away
- Staff reports of "the wheel moved on its own"

**Prevention:**
1. NEVER apply spring-centering or any force without a ramp-up period. Use `axis.idlespring` at low value first (10%), ramp to target over 500ms-1s.
2. All preset loads must go through a "force-off -> load preset -> gradual enable" sequence
3. Implement a "hands-off" interlock: do not enable FFB until driving input is detected (steering angle change or pedal input from `driving_detector.rs`)
4. Cap force output at the wheelbase level: set `axis.power` to a venue-safe maximum (e.g., 70% of 8Nm = 5.6Nm) -- enough for immersion, much less dangerous for casual customers
5. Keep the venue waiver updated, but never rely on it as the primary safety measure

**Phase:** Phase 1 (safety is non-negotiable and must be designed into the session lifecycle from day one).

**Confidence:** HIGH -- injury reports are well-documented in the sim racing community (SimXPro, Boosted Media, OverTake.gg forums).

---

### Pitfall 4: Force-Killing ConspitLink2.0.exe Corrupts State

**What goes wrong:** When ConspitLink is terminated via `taskkill /F` (or crashes), it does not flush its in-memory state to disk. JSON config files may be left in an inconsistent state: half-written presets, corrupted `Settings.json`, or stale `Global.json` values. On next launch, ConspitLink either fails to start, loads wrong presets, or ignores auto-switch config.

**Why it happens:** ConspitLink (like most desktop apps) writes config on graceful shutdown. Force-kill bypasses this. Additionally, if ConspitLink is mid-write when killed, the JSON file may be truncated or contain invalid data.

**Consequences:**
- Pod boots with wrong FFB profile (dangerous if it loads a high-force profile for wrong game)
- ConspitLink fails to start on next boot, requiring manual intervention on that pod
- Fleet drift: pods silently diverge from the intended configuration
- `AresAutoChangeConfig` stops working because `Global.json` is corrupted

**Warning signs:**
- ConspitLink takes unusually long to start or shows errors on launch
- A pod behaves differently from the other 7
- JSON files have 0-byte size or parse errors
- `ensure_conspit_link_running()` triggers restarts more than once per session

**Prevention:**
1. NEVER use `taskkill /F` on ConspitLink. Use graceful shutdown: `WM_CLOSE` message to the window handle, then wait up to 10s for process exit, only then escalate.
2. Keep backup copies of all JSON configs. Before any fleet push, snapshot the current state. After ConspitLink starts, verify JSON integrity.
3. Implement a config validator in rc-agent that checks JSON parse-ability and key field presence before and after ConspitLink restarts.
4. If ConspitLink is hung (not responding to WM_CLOSE), as a last resort: save a known-good config copy, force-kill, restore the config copy, then restart.

**Phase:** Phase 1 (the existing `ensure_conspit_link_running()` watchdog needs to be hardened immediately).

**Confidence:** HIGH -- documented in PROJECT.md constraints: "ConspitLink crashes on force-kill."

---

## Moderate Pitfalls

---

### Pitfall 5: Auto Game Detection Race Condition

**What goes wrong:** ConspitLink's `AresAutoChangeConfig: "open"` feature detects a game process and loads the matching preset. But the game may launch its FFB initialization BEFORE ConspitLink finishes loading the preset. The game downloads effects to the wheelbase using the OLD preset's parameters (wrong rotation angle, wrong force limits). Result: mismatched FFB feel, or worse, 900-degree rotation config applied to a game expecting 360 degrees.

**Why it happens:** Process detection is inherently racy. ConspitLink polls for game processes on an interval (likely 1-5s). The game's DirectInput device acquisition happens immediately on launch. There is no synchronization protocol between them.

**Consequences:**
- Steering rotation mismatch (900 vs 360 degrees) -- customer spins wildly or barely turns
- Force limits from previous game applied to new game
- Customer thinks the equipment is broken
- Staff confusion debugging intermittent "wrong feel" reports

**Warning signs:**
- First 5-10 seconds of a session feel "off" then correct themselves
- Steering lock is wrong for the first lap
- ConspitLink log shows preset load AFTER game's FFB init

**Prevention:**
1. Do NOT rely on ConspitLink's built-in auto-switch. rc-agent already knows which game is launching (it manages the kiosk). Have rc-agent load the correct preset BEFORE launching the game.
2. Sequence: (a) rc-agent loads preset via ConspitLink config -> (b) verify preset is active (read back from HID or check ConspitLink state) -> (c) THEN launch game executable.
3. If ConspitLink's auto-switch is used as a fallback, add a post-launch verification step: 2-3 seconds after game launch, confirm the active preset matches the expected one.
4. For games that init FFB very aggressively (F1 25, Codemasters titles), add a 1-2s delay between preset load and game launch.

**Phase:** Phase 2 (auto game-profile switching). This is the core design decision for that phase.

**Confidence:** MEDIUM -- the race condition is a known pattern in wheelbase software. ConspitLink-specific timing not directly verified, but `AresAutoChangeConfig` is documented as "broken" in PROJECT.md which is consistent with this pitfall.

---

### Pitfall 6: UDP Port Conflicts Between ConspitLink, SimHub, and Game Telemetry

**What goes wrong:** Multiple applications try to bind to the same UDP port for telemetry data. F1 25 sends telemetry to port 20778. ConspitLink also listens on 20778 (per Settings.json). If SimHub or any other telemetry consumer is added later, it cannot bind to that port. Only one application can listen on a given UDP port.

**Why it happens:** UDP has no connection negotiation. The first process to `bind()` wins. The second gets `WSAEADDRINUSE` and either silently fails or crashes. Games typically send to a single configured port with no built-in forwarding.

**Consequences:**
- ConspitLink gets no telemetry (wheel display dead, shift lights dead, RPM LEDs dead)
- Or SimHub gets no telemetry (dashboards dead)
- Silent failure: the losing application shows no error, just shows stale/zero data
- Debugging is frustrating because the problem depends on process start order

**Warning signs:**
- Telemetry features work sometimes but not always
- Restarting a specific application "fixes" it (because it now binds first)
- `netstat -ano | findstr 20778` shows the wrong PID owning the port

**Prevention:**
1. Establish a clear port ownership chain: Game -> Primary listener (ConspitLink at 20778) -> Forward to secondary consumers
2. If SimHub is added later, configure it on a different port (e.g., 20779) and have the primary listener forward
3. Document the port map for the fleet and enforce it via rc-agent config validation
4. On pod startup, verify port ownership with `netstat` before declaring the pod healthy
5. For shared-memory games (AC, ACC, AC EVO), this is not an issue -- shared memory supports multiple readers. Only UDP-based games (F1 25) have this problem.

**Phase:** Phase 2-3 (telemetry dashboard and shift lights). Must be designed correctly from the start.

**Confidence:** HIGH -- UDP port exclusivity is a fundamental networking constraint. Port 20778 conflict documented in Settings.json.

---

### Pitfall 7: Fleet Config Drift and Preset Corruption During Push

**What goes wrong:** When pushing FFB presets and JSON configs to all 8 pods, some pods end up with different settings than others. This happens because: (a) ConspitLink is running during the push and locks files, (b) a preset file is partially written when ConspitLink reads it, (c) network/USB hiccup during push leaves one pod behind, or (d) a pod was in sleep mode (blue indicator) and missed the update.

**Why it happens:** File-based configuration with no transactional semantics. ConspitLink reads configs opportunistically. There is no version number or checksum in the config files to detect drift.

**Consequences:**
- Pod 3 has different FFB strength than pods 1-2, 4-8
- Customer complains "this rig feels different" -- hard to diagnose
- Over time, manual tweaks accumulate and no pod matches the "golden config"
- Preset corruption causes ConspitLink to fall back to defaults silently

**Warning signs:**
- Customer complaints about inconsistent feel between pods
- JSON file sizes differ across pods for the same config
- A pod loads "default.Base" instead of the venue-tuned preset

**Prevention:**
1. Define a "golden config" directory in the repo. All configs are version-controlled.
2. Push protocol: (a) stop ConspitLink gracefully, (b) write all config files atomically (write to temp, rename), (c) verify checksums, (d) restart ConspitLink, (e) verify active preset via HID readback.
3. Add a config hash to rc-agent's health monitoring. If a pod's config hash diverges from golden, alert and auto-remediate.
4. Never push to a pod that is in sleep mode (blue power indicator). Wake it first, verify USB connectivity, then push.
5. Encrypt preset files use `.conspit` format -- ensure the push process handles these binary files correctly (binary copy, not text mode).

**Phase:** Phase 3 (fleet-wide config push via rc-agent). This is the core design challenge for that phase.

**Confidence:** MEDIUM -- inferred from the file-based config architecture. Not directly observed at scale yet since fleet push is not yet implemented.

---

### Pitfall 8: Firmware Update Failures Across Fleet (Bricking Risk)

**What goes wrong:** Updating OpenFFBoard firmware on 8 wheelbases simultaneously or sequentially, one fails mid-flash. The device is now in DFU mode (or worse, a bad state) and requires physical access with STM32CubeProgrammer + Zadig driver to recover. If the update was pushed remotely, the pod is bricked until someone physically intervenes.

**Why it happens:** Firmware updates over USB are inherently risky. A USB disconnect, power glitch, or Windows driver hiccup during the 30-60 second flash window can corrupt the firmware. OpenFFBoard uses STM32 DFU which is recoverable but requires specific tooling.

**Consequences:**
- One or more pods out of service until physical recovery
- Recovery requires Zadig driver install + STM32CubeProgrammer (not something rc-agent can automate)
- If multiple pods brick, significant venue downtime
- Firmware version mismatch across fleet causes inconsistent behavior

**Warning signs:**
- Wheelbase USB device disappears after update attempt
- Device shows up as "STM32 BOOTLOADER" in Device Manager instead of OpenFFBoard
- Power indicator does not return to green after update

**Prevention:**
1. NEVER update all pods simultaneously. Update one pod, verify it works (full FFB test cycle), then proceed to the next. Maximum 2 pods down at any time.
2. Before any firmware update, record the current firmware version per pod. If the update fails, you know exactly which version to restore.
3. Keep the DFU recovery tools (Zadig, STM32CubeProgrammer, last-known-good .hex file) pre-installed on every pod PC. Document the recovery procedure step-by-step for staff.
4. Do NOT automate firmware updates via rc-agent in v1. This is a manual, supervised operation. Automate only the version-check reporting.
5. Schedule firmware updates during maintenance windows, never during business hours.

**Phase:** Out of scope for v1. Add version monitoring in Phase 3 (fleet management), but actual firmware updates should remain manual.

**Confidence:** MEDIUM -- based on OpenFFBoard wiki DFU documentation. Conspit may have their own firmware update tool that adds another layer of complexity.

---

## Minor Pitfalls

---

### Pitfall 9: ConspitLink Window Focus Stealing Breaks Kiosk Mode

**What goes wrong:** ConspitLink occasionally pops to foreground (update prompts, error dialogs, first-launch wizards), stealing focus from the kiosk lock screen. Customers see the ConspitLink UI instead of the Racing Point branded experience. Worse, they can interact with it and change settings.

**Prevention:**
1. Maintain the existing `minimize_conspit_window()` approach but run it on a faster interval during game transitions (when ConspitLink is most likely to show dialogs).
2. Disable ConspitLink's auto-update check via config (if available) or firewall rule.
3. On fresh installs, complete all first-run wizards before deploying to production.

**Phase:** Already partially handled by rc-agent. Harden in Phase 1.

---

### Pitfall 10: Encrypted .conspit Preset Files Cannot Be Edited Programmatically

**What goes wrong:** ConspitLink stores some presets as encrypted `.conspit` binary files. You cannot read, modify, or validate these files from rc-agent. If you need to tweak a pro driver preset parameter, you must use the ConspitLink GUI, export, and re-deploy.

**Prevention:**
1. For venue-tuned presets, always use the `.Base` JSON format (which IS human-readable and editable).
2. Treat `.conspit` files as opaque blobs -- copy them as-is during fleet push, never try to parse.
3. Build your own preset library entirely in `.Base` format so you have full control.
4. Only use `.conspit` presets as reference/starting points, not as production presets.

**Phase:** Phase 2 (per-game FFB preset tuning). Design decision: all Racing Point presets in `.Base` format.

---

### Pitfall 11: Multiple Processes Writing to ConspitLink JSON Config Simultaneously

**What goes wrong:** rc-agent writes to `Global.json` to set auto-switch config. ConspitLink reads the same file. If rc-agent writes while ConspitLink is mid-read (or vice versa), one gets a partial/corrupt file. No file locking is implemented.

**Prevention:**
1. All config writes must go through a single writer (rc-agent). ConspitLink should be read-only consumer.
2. Before writing, verify ConspitLink is not actively loading (check process state / wait for idle).
3. Use atomic write pattern: write to `.tmp` file, then `rename()` (which is atomic on NTFS).
4. After write, poll for ConspitLink to acknowledge the new config (behavioral verification, not file-level).

**Phase:** Phase 1 (config management hardening) and Phase 3 (fleet push protocol).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Phase 1: Stuck-rotation fix | P-20 race between rc-agent HID and ConspitLink | Work WITH ConspitLink preset system, not against it via raw HID |
| Phase 1: Stuck-rotation fix | ESTOP cuts centering spring too | Use `fxm.reset` + `axis.idlespring` instead of `main.estop` for routine session end |
| Phase 1: Safety | Snap-back torque on centering | Ramp-up forces gradually, never instant-apply |
| Phase 2: Auto game-profile switch | Game inits FFB before preset loads | rc-agent loads preset BEFORE launching game, not after |
| Phase 2: FFB preset tuning | Encrypted .conspit files not editable | Build all venue presets in .Base JSON format |
| Phase 2: Telemetry dashboards | UDP port 20778 contention | Clear port ownership chain, single listener + forwarding |
| Phase 3: Fleet config push | Partial writes corrupt config | Atomic write + checksum verify + stop ConspitLink during push |
| Phase 3: Fleet config push | Pod in sleep mode misses update | Wake + verify USB before push |
| Phase 3: Firmware version monitoring | Temptation to automate firmware updates | Do NOT automate in v1, monitor versions only |
| All phases | Force-killing ConspitLink | Always graceful shutdown (WM_CLOSE), never taskkill /F |

## Sources

- [OpenFFBoard Commands Wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Commands) -- effect reset, ESTOP, idle spring, FFB state commands (HIGH confidence)
- [OpenFFBoard Setup Wiki](https://github.com/Ultrawipf/OpenFFBoard/wiki/Setup) -- DFU mode, firmware recovery (HIGH confidence)
- [OpenFFBoard Games Setup](https://github.com/Ultrawipf/OpenFFBoard/wiki/Games-setup) -- DirectInput FFB behavior per game (MEDIUM confidence)
- [DCS Force Feedback Fix (GitHub)](https://github.com/walmis/dcs-force-feedback-fix) -- DirectInput8 proxy DLL approach to FFB reconnection/cleanup (MEDIUM confidence, different game but same DirectInput mechanics)
- [SimXPro Direct Drive Safety Guide](https://simxpro.com/blogs/guides/direct-drive-safety-torque-limits-wrist-injuries-and-the-emergency-stop) -- torque injury risks, emergency stop design (HIGH confidence)
- [Boosted Media: Sim Racing Safety Tips](https://boostedmedia.net/sim-racing/how-to-videos-sim-racing/these-arent-toys-sim-racing-safety-tips/) -- documented hand/wrist injury incidents (MEDIUM confidence)
- [OverTake.gg: Risk of Injury with DD Motors](https://www.overtake.gg/threads/risk-of-injury-with-dd-motors.187609/) -- community reports of finger/wrist injuries (MEDIUM confidence)
- [MOZA Port Conflict Guide](https://support.mozaracing.com/en/support/solutions/articles/70000628623-the-game-plug-in-s-port-conflicts-with-the-telemetry-data-s-port) -- UDP telemetry port conflict patterns (MEDIUM confidence, different vendor but same problem)
- [SimHub UDP Forwarding Forum](https://www.simhubdash.com/community-2/simhub-support/using-udp-forwarding-feature-for-f1-22-telemetry-on-two-devices-help/) -- forwarding chain setup (MEDIUM confidence)
- [GameDev.net: DirectInput FFB Focus Loss](https://www.gamedev.net/forums/topic/711649-properly-re-acquiring-a-directinput-device-for-force-feedback-on-focus-loss/) -- orphaned effects on process/focus change (MEDIUM confidence)
- PROJECT.md -- P-20, stuck-rotation bug, ConspitLink crash behavior, fleet constraints (HIGH confidence, primary source)
