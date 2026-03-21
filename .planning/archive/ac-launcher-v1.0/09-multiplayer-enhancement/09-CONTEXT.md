# Phase 9: Multiplayer Enhancement - Context

**Gathered:** 2026-03-14
**Status:** Ready for planning

<domain>
## Phase Boundary

Multi-pod multiplayer races with AI grid fillers, synchronized billing, and a lobby experience. Multiple customers on different pods join the same race on the AC dedicated server running on Racing-Point-Server (.23). AI fills remaining grid spots. Billing starts/stops simultaneously for all participants. PWA shows a lobby/waiting screen with who has joined and race status. Multiplayer uses existing ac_server.rs infrastructure (generate_server_cfg_ini, generate_entry_list_ini, start_ac_server).

</domain>

<decisions>
## Implementation Decisions

### AI Grid Fillers
- **Same car as players** — all AI drive the same car model the host chose. Fair racing, no advantage from car choice.
- **Auto-fill to track max** — AI fills all remaining grid spots up to the track's pit count minus human players. No user choice on AI count.
- **Host's difficulty setting** — AI fillers use the AI_LEVEL from the host's chosen difficulty tier (Rookie/Amateur/Semi-Pro/Pro/Alien).
- **Entry list INI approach** — Add AI entries directly in entry_list.ini with empty GUIDs. Standard AC approach, no CSP server plugin dependency. Use existing 60-name AI driver pool from Phase 1.

### Synchronized Billing Flow
- **Billing starts when last player is on-track** — billing starts for ALL players only after every participant's AC shared memory reports STATUS=LIVE. Consistent with Phase 3's single-player billing trigger.
- **Disconnected player billing stops individually** — if a player disconnects mid-race, their billing stops. Remaining players continue racing and billing normally.
- **Race ends naturally** — when the AC race finishes (laps complete or time expires), billing stops for all participants simultaneously.
- **Each player pays their own** — each participant's wallet is debited independently. Existing book_multiplayer debits host; invitees pay on accept. Keep this pattern.

### Lobby & Race Countdown
- **Auto-start when all checked in** — race launches automatically on all pods when every invited player's status is 'validated' (checked in at their pod). No manual host trigger.
- **Show track, car, and AI count** — lobby displays selected track name, car model, number of AI opponents alongside existing member list with check-in status.
- **Status text only** — "Waiting for 2 more players..." → "All players checked in!" → "Race starting on all pods..." No countdown timer — existing pattern is sufficient.
- **Show pod number per player** — each member row shows "Pod 3" so players know which physical pod to sit at. Already partially in the UI (m.pod_number).

### Server-to-Pod Launch Flow
- **Content Manager URI auto-join** — rc-agent launches AC via acmanager:// URI with server IP/port. Player auto-joins the server. Already researched in Phase 1.
- **60-second connection timeout** — if a pod fails to connect within 60s, marked as disconnected. Other players proceed. Failed player's billing doesn't start.
- **AC server on Racing-Point-Server (.23)** — dedicated server runs on the central server machine where rc-core runs. No load on player pods.
- **All same car** — host picks the car, everyone drives it. Same car as AI fillers.

### Claude's Discretion
- WebSocket message flow for multiplayer state coordination between rc-core and rc-agent
- How to detect "all players on-track" for synchronized billing start (polling vs event-driven)
- acmanager:// URI format and parameters for server auto-join
- How to coordinate the "all validated → launch on all pods" transition
- Error handling for partial server starts or mid-session failures

</decisions>

<specifics>
## Specific Ideas

- Existing multiplayer.rs already has book_multiplayer(), find_adjacent_idle_pods(), shared PIN, wallet debit, pod reservation — this is ~50% of the orchestration.
- Existing ac_server.rs has generate_server_cfg_ini(), generate_entry_list_ini(), start_ac_server() — the server management infrastructure is built.
- Existing PWA /book/multiplayer page has friend selection, track/car selection, confirm step — needs session type integration and AI config.
- Existing PWA /book/group page has member list with status polling (3s interval), accept/decline, shared PIN display — needs track/car/AI info.
- The "all validated" status trigger already exists in group page — `group.status === "all_validated"` displays "Race starting on all pods..."
- AI entries in entry_list.ini should have DRIVERNAME from the existing 60-name pool but empty GUID (AC treats empty GUID as AI).

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `ac_server.rs` (rc-core): AcServerManager, generate_server_cfg_ini, generate_entry_list_ini, start_ac_server — full server lifecycle management
- `multiplayer.rs` (rc-core): book_multiplayer, find_adjacent_idle_pods, shared PIN generation, wallet debit, pod reservation
- `AcLanSessionConfig` (rc-common/types.rs): Full server config struct with entries, sessions, weather, dynamic track
- `EntryListEntry` (rc-common/types.rs): car_model, skin, driver_name, guid, ballast, restrictor — for both human and AI entries
- `/book/multiplayer/page.tsx` (PWA): 3-step wizard (Friends → Configure → Confirm) with friend selection, tier/track/car picker
- `/book/group/page.tsx` (PWA): Lobby with member list, status polling, shared PIN, accept/decline
- `get_driver_entry_info()` (ac_server.rs): Resolves driver name and GUID for entry list — extend for AI entries
- `AI_DRIVER_NAMES` (ac_launcher.rs): 60-name international pool for AI — reuse for server entry list

### Established Patterns
- WebSocket protocol: AgentMessage/CoreMessage adjacently-tagged enums — add multiplayer variants
- Billing: STATUS=LIVE triggers billing start (Phase 3) — extend to multi-pod synchronization
- Pod state: PodStatus enum with Idle/InGame/etc. — track multiplayer state
- Launch flow: CoreToAgentMessage::LaunchGame sends launch_args JSON to agent — extend with server join info

### Integration Points
- rc-core multiplayer.rs: When all players validate → call start_ac_server → send LaunchGame to all pods with acmanager:// URI
- rc-agent main.rs: Handle LaunchGame with multiplayer flag → launch AC with acmanager:// URI instead of direct acs.exe
- rc-core billing: Coordinate multi-pod billing start (all STATUS=LIVE) and stop (race end or disconnect)
- PWA /book/group: Enhance lobby with track/car/AI info from group session data

</code_context>

<deferred>
## Deferred Ideas

- Race Weekend multiplayer (group Practice → Qualify → Race sequence) — AMLT-01, v2
- Spectator mode for waiting customers — AMLT-02, v2
- Custom livery selection per customer — AMLT-03, v2
- AI grid size slider in multiplayer config — decided against (auto-fill to max)

</deferred>

---

*Phase: 09-multiplayer-enhancement*
*Context gathered: 2026-03-14*
