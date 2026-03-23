# E2E Test Results — {DATE}

**Tester:** _______________
**Date:** {DATE}
**Environment:** POS (:3200) + Kiosk (:3300) on 192.168.31.23
**Automated runner:** `bash test/e2e/run-e2e.sh` → see E2E-TEST-RESULTS-{DATE}.md
**Total tests:** 231

---

## Run Summary

| Section | Total | Pass | Fail | Skip | Notes |
|---------|-------|------|------|------|-------|
| 1.1 Login | 4 | | | | |
| 1.2 Sidebar Nav | 22 | | | | |
| 1.3 Live Overview | 6 | | | | |
| 1.4 Games | 9 | | | | |
| 1.5 Billing | 13 | | | | |
| 1.6 AC LAN | 9 | | | | |
| 1.7 Leaderboards | 8 | | | | |
| 1.8 Cameras | 16 | | | | |
| 1.9 Cafe Menu | 9 | | | | |
| 1.10 AI Insights | 5 | | | | |
| 1.11 Settings | 4 | | | | |
| 1.12 Drivers | 3 | | | | |
| 1.13 Presenter | 2 | | | | |
| 2.1 Kiosk Landing | 9 | | | | |
| 2.2 PIN Entry | 9 | | | | |
| 2.3 Booking Wizard | 25 | | | | |
| 2.4 Pod Kiosk | 19 | | | | |
| 2.5 Staff Control | 20 | | | | |
| 2.6 Fleet Health | 12 | | | | |
| 2.7 Spectator | 6 | | | | |
| 3.1 Responsiveness | 5 | | | | |
| 3.2 Real-Time | 5 | | | | |
| 3.3 Error Handling | 5 | | | | |
| 3.4 Edge Cases | 6 | | | | |
| **TOTAL** | **231** | | | | |

> **Note:** Automated tests (page loads + API checks) are recorded in E2E-TEST-RESULTS-{DATE}.md from the runner.
> Only manual UI interaction tests appear as checkboxes below.

---

## Manual Test Checklist

### 1.1 Login

- [ ] **1.1.2** — Enter wrong PIN → Error message shown, stays on login
- [ ] **1.1.3** — Enter correct PIN → Redirects to Live Overview (`/`)
- [ ] **1.1.4** — Refresh page after login → Stays logged in (session persists)

### 1.2 Sidebar Navigation

- [ ] **1.2.1** — Click every sidebar link → Each page loads without error
- [ ] **1.2.2** — Live Overview → Pod grid visible (4-col), connection status shown
- [ ] **1.2.3** — Pods → Pod listing with online/offline status
- [ ] **1.2.4** — Games → Pod grid with Launch/Stop game buttons
- [ ] **1.2.5** — Telemetry → Speed, RPM, Throttle, Brake bars visible
- [ ] **1.2.6** — AC LAN → Pod checkboxes, Track/Car dropdowns, session config
- [ ] **1.2.7** — AC Results → Session list loads
- [ ] **1.2.8** — Sessions → Session list with status/track/sim type
- [ ] **1.2.9** — Drivers → Driver grid (3-col) with avatars
- [ ] **1.2.10** — Leaderboards → Tab navigation (Records/Drivers/Tracks)
- [ ] **1.2.11** — Events → Event list loads
- [ ] **1.2.12** — Billing → Pod grid with Start/Pause/Extend/End buttons
- [ ] **1.2.13** — Pricing → Pricing tiers visible
- [ ] **1.2.14** — History → Billing history loads
- [ ] **1.2.15** — Bookings → Booking list loads
- [ ] **1.2.16** — AI Insights → Filter buttons (All/Active/Dismissed)
- [ ] **1.2.17** — Cameras → Camera grid with mode buttons
- [ ] **1.2.18** — Playback → Playback page loads
- [ ] **1.2.19** — Cafe Menu → Tabs (Items/Inventory/Promos) visible
- [ ] **1.2.20** — Settings → Server status, Venue info, POS Lockdown toggle
- [ ] **1.2.21** — Presenter View → Opens presenter display
- [ ] **1.2.22** — Kiosk Mode link → Opens kiosk app

### 1.3 Live Overview

- [ ] **1.3.1** — Pod grid renders → All pods shown with correct status colors
- [ ] **1.3.2** — Idle pod shows green → Available pods are green/idle
- [ ] **1.3.3** — Active pod shows red → In-session pods are red/active
- [ ] **1.3.4** — Offline pod shows dimmed → Offline pods are greyed out
- [ ] **1.3.5** — Telemetry bar updates → Live speed/RPM on active pods
- [ ] **1.3.6** — Lap feed scrolls → Recent laps appear in real-time

### 1.4 Games

- [ ] **1.4.1** — Click "Launch Game" on idle pod → Game selection modal opens
- [ ] **1.4.2** — Modal shows game options → AC, iRacing, F1 25, Le Mans Ultimate, Forza visible
- [ ] **1.4.3** — Select a game → Game highlighted
- [ ] **1.4.4** — Click Launch → Game launches, pod status changes to "launching"
- [ ] **1.4.5** — Wait for launch complete → Pod status changes to "running", game name shown
- [ ] **1.4.6** — Click "Stop Game" on running pod → Game stops, pod returns to idle
- [ ] **1.4.7** — Click Launch on offline pod → Button disabled or error shown
- [ ] **1.4.8** — Close modal with X → Modal closes, no action taken
- [ ] **1.4.9** — Close modal with Escape → Modal closes

### 1.5 Billing

- [ ] **1.5.1** — Click "Start" on idle pod → Billing start modal opens
- [ ] **1.5.2** — Mode tabs visible → PIN/QR/Direct tabs
- [ ] **1.5.3** — Search for driver → Driver dropdown populates
- [ ] **1.5.4** — Select pricing tier → Tier highlighted
- [ ] **1.5.5** — Click Start → Session starts, timer begins on pod card
- [ ] **1.5.6** — Session timer counts down → Timer decrements every second
- [ ] **1.5.7** — Click "Pause" on active session → Session pauses, button changes to "Resume"
- [ ] **1.5.8** — Click "Resume" → Session resumes, timer continues
- [ ] **1.5.9** — Click "Extend" (+10min) → Timer adds 10 minutes
- [ ] **1.5.10** — Click "End" on active session → Session ends, pod returns to idle
- [ ] **1.5.11** — Session < 2min remaining → Warning indicator appears
- [ ] **1.5.12** — Variable time toggle → Custom duration/price fields appear
- [ ] **1.5.13** — Start without selecting driver → Validation error shown

### 1.6 AC LAN Race

- [ ] **1.6.1** — Pod checkboxes → Can select/deselect individual pods
- [ ] **1.6.2** — Track dropdown → Lists available tracks, can select
- [ ] **1.6.3** — Car dropdown → Lists available cars, can select
- [ ] **1.6.4** — Session type toggle → Practice/Qualifying/Race toggles correctly
- [ ] **1.6.5** — Duration/laps input → Can enter custom values
- [ ] **1.6.6** — Advanced settings → Expand/collapse works
- [ ] **1.6.7** — Load preset → Preset loads, fields populate
- [ ] **1.6.8** — Save preset → New preset saved
- [ ] **1.6.9** — Click Start → Race launches on selected pods

### 1.7 Leaderboards

- [ ] **1.7.1** — Records tab → Shows fastest times
- [ ] **1.7.2** — Drivers tab → Shows driver rankings
- [ ] **1.7.3** — Tracks tab → Shows track records
- [ ] **1.7.4** — Sim type filter → Filters by AC/iRacing/F1 etc.
- [ ] **1.7.5** — "Show Invalid" toggle → Toggles invalid laps visibility
- [ ] **1.7.6** — Car filter (per track) → Filters leaderboard by car
- [ ] **1.7.7** — Position colors → 1st=gold, 2nd=silver, 3rd=bronze
- [ ] **1.7.8** — Track drill-down → Click track shows detailed times

### 1.8 Cameras

- [ ] **1.8.2** — Grid mode buttons (1/4/9/16) → Grid layout changes per selection
- [ ] **1.8.3** — Refresh rate dropdown → Changes polling interval
- [ ] **1.8.4** — Status dots → Green=live, Yellow=stale, Red=offline
- [ ] **1.8.5** — Zone colors → Red=entrance, Blue=reception, Green=pods, Grey=other
- [ ] **1.8.6** — Click tile → fullscreen → Fullscreen overlay opens with live video
- [ ] **1.8.7** — Fullscreen close (X button) → Returns to grid
- [ ] **1.8.8** — Fullscreen close (Escape) → Returns to grid
- [ ] **1.8.9** — Fullscreen close (backdrop click) → Returns to grid
- [ ] **1.8.10** — Fullscreen prev/next (arrow keys) → Cycles to adjacent camera
- [ ] **1.8.11** — Fullscreen prev/next (on-screen buttons) → Hover edges shows buttons, click cycles
- [ ] **1.8.12** — Grid keyboard nav (arrow keys) → Red outline moves between tiles
- [ ] **1.8.13** — Grid keyboard nav (Enter) → Opens fullscreen on focused tile
- [ ] **1.8.14** — Drag-and-drop reorder → Tiles swap positions, layout saved
- [ ] **1.8.15** — Controls auto-hide (3s) → Fullscreen controls fade after 3s
- [ ] **1.8.16** — Controls reappear on mousemove → Controls show again

### 1.9 Cafe Menu

- [ ] **1.9.1** — Items tab → Menu items list visible
- [ ] **1.9.2** — Inventory tab → Stock levels shown
- [ ] **1.9.3** — Promos tab → Promotions list
- [ ] **1.9.4** — Add new item → Form opens, can enter name/price/category
- [ ] **1.9.5** — Edit existing item → Fields populate, can modify
- [ ] **1.9.6** — Stock status badges → Out/Low/Warning/In Stock shown correctly
- [ ] **1.9.7** — Category dropdown → Can filter by category
- [ ] **1.9.8** — Promo on/off toggle → Toggle works, status changes
- [ ] **1.9.9** — Promo types → Combo/Happy Hour/Bundle selectable

### 1.10 AI Insights

- [ ] **1.10.1** — Filter: All → Shows all insights
- [ ] **1.10.2** — Filter: Active → Shows only active
- [ ] **1.10.3** — Filter: Dismissed → Shows only dismissed
- [ ] **1.10.4** — Refresh button → Reloads insights
- [ ] **1.10.5** — Dismiss insight → Card moves to dismissed

### 1.11 Settings

- [ ] **1.11.1** — Server status → Shows health, version
- [ ] **1.11.2** — Venue info → Name, location, timezone, capacity shown
- [ ] **1.11.3** — POS Lockdown toggle → Toggles between Locked/Unlocked
- [ ] **1.11.4** — POS Lockdown effect → When locked, verify restricted actions

### 1.12 Drivers

- [ ] **1.12.1** — Driver grid loads → 3-column grid with avatar initials
- [ ] **1.12.2** — Driver info shown → Email, total laps, track time
- [ ] **1.12.3** — Long name truncation → Names don't overflow card

### 1.13 Presenter View

- [ ] **1.13.1** — Pod counts → Active/Idle/Total shown
- [ ] **1.13.2** — Live lap feed → Full-width lap ticker updates

---

## PART 2: Kiosk (:3300)

### 2.1 Customer Landing

- [ ] **2.1.2** — Available pods show green → Idle pods highlighted as available
- [ ] **2.1.3** — Racing pods show red → In-session pods shown as occupied
- [ ] **2.1.4** — Click available pod → PIN entry modal opens
- [ ] **2.1.5** — Click occupied pod → Nothing happens / "In use" indicator
- [ ] **2.1.6** — "Book a Session" button → Navigates to `/book`
- [ ] **2.1.7** — "Have a PIN?" button → Opens PIN entry
- [ ] **2.1.8** — Staff Login link → Navigates to `/staff`
- [ ] **2.1.9** — Live status indicator → Shows real-time connection status

### 2.2 PIN Entry Modal

- [ ] **2.2.1** — Numpad renders → 1-9, 0, Clear, Backspace buttons visible
- [ ] **2.2.2** — Press digits → Dots fill up (4 dots max)
- [ ] **2.2.3** — Backspace → Removes last digit
- [ ] **2.2.4** — Clear → Clears all digits
- [ ] **2.2.5** — Auto-submit at 4 digits → Validation triggers automatically
- [ ] **2.2.6** — Correct PIN → Success state, pod assigned, session starts
- [ ] **2.2.7** — Wrong PIN → Error message, can retry
- [ ] **2.2.8** — 60s inactivity → Modal auto-closes
- [ ] **2.2.9** — Only numeric input → Non-numeric keys ignored

### 2.3 Booking Wizard

- [ ] **2.3.1** — Phone input field → Can enter phone number
- [ ] **2.3.2** — Invalid phone format → Validation error shown
- [ ] **2.3.3** — Send OTP button → OTP sent, advances to OTP screen
- [ ] **2.3.4** — 6-digit OTP input → Can enter OTP code
- [ ] **2.3.5** — Correct OTP → Advances to setup wizard
- [ ] **2.3.6** — Wrong OTP → Error message shown
- [ ] **2.3.7** — Resend OTP link → New OTP sent (check throttle ~30s)
- [ ] **2.3.8** — Select Plan → Pricing tier cards visible, can select
- [ ] **2.3.9** — Select Game → Game grid shows (AC/iRacing/F1/etc), can select
- [ ] **2.3.10** — Player Mode → Solo/Multiplayer options, can select
- [ ] **2.3.11** — Session Type → Practice/Qualifying/Race tabs work
- [ ] **2.3.12** — AI Opponents → Difficulty presets + AI count slider
- [ ] **2.3.13** — Select Experience → Experience cards visible, can select
- [ ] **2.3.14** — Select Track → Track search works, category filter, can select
- [ ] **2.3.15** — Select Car → Car search works, category filter, can select
- [ ] **2.3.16** — Driving Settings → Controller layout, ABS toggle, TC slider
- [ ] **2.3.17** — Review & Confirm → Summary shows all selections, Confirm button
- [ ] **2.3.18** — Skip button → Skips optional steps correctly
- [ ] **2.3.19** — Back button → Returns to previous step, retains state
- [ ] **2.3.20** — Next button validates → Can't proceed without required selection
- [ ] **2.3.21** — PIN code displayed → 4-digit PIN shown clearly
- [ ] **2.3.22** — Pod number shown → Assigned pod highlighted
- [ ] **2.3.23** — Allocated time shown → Session duration displayed
- [ ] **2.3.24** — Done button → Returns to landing
- [ ] **2.3.25** — Auto-return (30s) → Landing page after 30s inactivity

### 2.4 Pod Kiosk View

- [ ] **2.4.1** — Experience grid (idle) → Game experiences shown as cards
- [ ] **2.4.2** — Click experience → Game selected/highlighted
- [ ] **2.4.3** — Launch button → Game begins launching
- [ ] **2.4.4** — Game splash screen (launching) → Shows game name + progress indicator
- [ ] **2.4.5** — "Setting up your rig..." → Spinner visible during setup
- [ ] **2.4.6** — Session timer (HH:MM:SS) → Counts down correctly, updates every second
- [ ] **2.4.7** — Speed display → Real-time speed value
- [ ] **2.4.8** — RPM display → Real-time RPM value
- [ ] **2.4.9** — Brake % display → Real-time brake pressure
- [ ] **2.4.10** — Lap count → Increments on lap completion
- [ ] **2.4.11** — Best lap time → Updates when new PB set
- [ ] **2.4.12** — Last lap time → Updates after each lap
- [ ] **2.4.13** — Game indicator badge → Shows correct game name
- [ ] **2.4.14** — Session warning (< 2min) → Warning indicator appears
- [ ] **2.4.15** — End Session button → Session ends, returns to idle/complete
- [ ] **2.4.16** — Completion message → "Session finished" shown
- [ ] **2.4.17** — Return link → Links back to booking
- [ ] **2.4.18** — Maintenance message → "Pod under maintenance" shown
- [ ] **2.4.19** — No interactive elements (disabled state) → All buttons disabled

### 2.5 Staff Login & Control

- [ ] **2.5.1** — PIN input (4 digits) → Staff PIN entry works
- [ ] **2.5.2** — Correct PIN → Shows staff name, redirects to `/control`
- [ ] **2.5.3** — Wrong PIN → Error message
- [ ] **2.5.4** — Pod grid (4x2) → All pods with status visible
- [ ] **2.5.5** — Pod card shows telemetry → Speed, RPM, brake % on active pods
- [ ] **2.5.6** — Session timer on active pod → Countdown visible
- [ ] **2.5.7** — Driver name on active pod → Correct driver shown
- [ ] **2.5.8** — Open game picker → Experience grid shown
- [ ] **2.5.9** — Launch game on pod → Game launches, status updates
- [ ] **2.5.10** — Session details panel → Active session info shown
- [ ] **2.5.11** — Pause/Resume → Session pauses and resumes
- [ ] **2.5.12** — Extend (+10min) → Timer adds 10 minutes
- [ ] **2.5.13** — End session → Session ends, pod returns to idle
- [ ] **2.5.14** — Driver selection for topup → Can select driver
- [ ] **2.5.15** — Topup amount input → Can enter amount
- [ ] **2.5.16** — Confirm topup → Balance updated
- [ ] **2.5.17** — Assistance alerts → Customer requests shown with dismiss
- [ ] **2.5.18** — Multiplayer group UI → Can create multiplayer groups
- [ ] **2.5.19** — Sign Out button → Returns to staff login
- [ ] **2.5.20** — 30min auto-logout → Inactive staff logged out automatically

### 2.6 Fleet Health

- [ ] **2.6.2** — Health status badges → Healthy/WS Only/HTTP Only/Offline/Maintenance
- [ ] **2.6.3** — WS connection indicator → Green if connected
- [ ] **2.6.4** — HTTP reachability indicator → Green if reachable
- [ ] **2.6.5** — Uptime display → Hours:minutes shown
- [ ] **2.6.6** — Violation count badge → 24h violation count
- [ ] **2.6.7** — Crash recovery indicator → Shows if pod recovered from crash
- [ ] **2.6.8** — Click Maintenance button → Maintenance modal opens
- [ ] **2.6.9** — Maintenance modal: PIN verify → PIN input validates staff
- [ ] **2.6.10** — Maintenance modal: failed checks → Lists what failed
- [ ] **2.6.11** — Maintenance modal: Clear button → Clears maintenance mode
- [ ] **2.6.12** — Maintenance modal: Close → Modal closes

### 2.7 Spectator View

- [ ] **2.7.2** — Live lap ticker → Laps appear in real-time
- [ ] **2.7.3** — Throttle/brake traces → Visualizations update
- [ ] **2.7.4** — Live leaderboard → Rankings shown with times
- [ ] **2.7.5** — Delta color coding → Positive=red, Negative=green
- [ ] **2.7.6** — Pod status cards → Real-time telemetry per pod

---

## PART 3: Cross-Cutting Tests

### 3.1 Responsiveness & Display

- [ ] **3.1.1** — Kiosk at 1920x1080 → All 8 pods visible, no scroll needed
- [ ] **3.1.2** — POS at full screen → Sidebar + content fit without overlap
- [ ] **3.1.3** — Modal sizing → Modals don't overflow screen
- [ ] **3.1.4** — Touch targets → All buttons >= 60px height on kiosk
- [ ] **3.1.5** — Long text truncation → Names/values don't break layout

### 3.2 Real-Time Updates

- [ ] **3.2.1** — Start session on POS → Kiosk shows pod as occupied immediately
- [ ] **3.2.2** — Launch game on POS → Pod kiosk view shows launching state
- [ ] **3.2.3** — End session on POS → Kiosk returns to idle
- [ ] **3.2.4** — Book on Kiosk → POS billing shows new session
- [ ] **3.2.5** — Telemetry during game → Both POS and Kiosk show live data

### 3.3 Error Handling

- [ ] **3.3.1** — Network disconnect → UI shows offline/reconnecting indicator
- [ ] **3.3.2** — Network reconnect → Data refreshes, status returns to normal
- [ ] **3.3.3** — API error on page load → Error message with Retry button
- [ ] **3.3.4** — Rapid button clicks → No duplicate actions (debounce works)
- [ ] **3.3.5** — Pod goes offline mid-game-launch → Error message, Retry or Clear option

### 3.4 Edge Cases

- [ ] **3.4.1** — Special characters in search → No crash, results filter correctly
- [ ] **3.4.2** — Empty state (no drivers) → "No data" placeholder shown
- [ ] **3.4.3** — Empty state (no sessions) → "No data" placeholder shown
- [ ] **3.4.4** — Empty state (no bookings) → "No data" placeholder shown
- [ ] **3.4.5** — Timezone shows IST → All times in IST
- [ ] **3.4.6** — Page refresh mid-session → Session state preserved

---

## Failures Log

Record each FAIL here with root cause:

| Test ID | Description | Error / Symptom | Root Cause | Fix Status |
|---------|-------------|-----------------|------------|------------|
| | | | | Fix committed: {hash} / Known issue: {ticket} |

---

## Known Issues

| Test ID | Description | Root Cause | Decision |
|---------|-------------|------------|----------|
| | | | Deferred / Won't fix / Fix in phase {N} |

---

## Sign-off

- [ ] All automated tests: PASS (see E2E-TEST-RESULTS-{DATE}.md)
- [ ] All manual tests above checked
- [ ] All FAILs have a Failures Log entry
- [ ] Phase 175 complete — update ROADMAP.md requirements E2E-01 through E2E-04
