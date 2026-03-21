---
phase: 93-community-tribal-identity
plan: "02"
subsystem: community-identity
tags: [tribal-identity, driver-language, whatsapp-bot, discord-bot, pwa]
dependency_graph:
  requires: []
  provides: [COMM-03]
  affects: [racingpoint-whatsapp-bot, racingpoint-discord-bot, racecontrol-pwa]
tech_stack:
  added: []
  patterns: [string-replacement, tribal-identity-language]
key_files:
  created: []
  modified:
    - /root/racingpoint-whatsapp-bot/src/prompts/systemPrompt.js
    - /root/racingpoint-whatsapp-bot/src/prompts/businessKnowledge.js
    - /root/racingpoint-discord-bot/src/prompts/systemPrompt.js
    - /root/racecontrol/pwa/src/app/register/page.tsx
    - /root/racecontrol/pwa/src/app/dashboard/page.tsx
decisions:
  - "Two extra user-facing 'Customer can ...' lines in WA systemPrompt.js BOOKING NOTES section replaced with 'Driver' (lines 156-157) — not in original plan count but clearly user-facing"
  - "One extra 'Customers can book' line in businessKnowledge.js line 105 replaced — not in original plan count of 2 but clearly user-facing booking instruction"
metrics:
  duration: "137s"
  completed: "2026-03-21T09:06:12Z"
  tasks_completed: 2
  files_modified: 5
---

# Phase 93 Plan 02: Driver Identity Language Copy Audit Summary

One-liner: Replace every user-facing "customer" with "driver" across WhatsApp bot, Discord bot, and PWA — making RacingPoint Driver the consistent tribal identity at all touchpoints.

## What Was Done

Executed a targeted copy audit across 3 repositories, replacing all user-facing occurrences of "customer" with "driver" (or "Driver") in bot system prompts, business knowledge, and PWA pages.

## Tasks

### Task 1: Replace "customer" with "driver" in bot prompts
**Commit (whatsapp-bot):** `de7c836`
**Commit (discord-bot):** `f3e0d32`

**WhatsApp systemPrompt.js** — 16 replacements total (14 planned + 2 extra booking flow lines):
- Match the driver's energy
- SAME LANGUAGE the driver writes in
- Read the driver's intent
- Regular driver asking "what's new"
- If a happy driver finishes a conversation
- When a driver wants to book a session
- If the driver says "I want to race"
- Ask the driver to confirm
- When a driver wants to book a package
- When a new driver wants to book
- If the driver is under 12
- If the driver is 12-17
- If a driver wants to book but
- mention this proactively to new drivers
- Driver can type a number (extra)
- Driver can say "cancel" (extra)

**WhatsApp businessKnowledge.js** — 3 replacements total (2 planned + 1 extra):
- If a driver requests a specific game
- Every registered driver gets a unique referral code
- Drivers can book through this chat (extra)

**Discord systemPrompt.js** — 2 replacements:
- SAME LANGUAGE the driver writes in
- "What's new" / Returning driver

Internal code identifiers preserved: `function buildSystemPrompt(customerContext)`, `const contextBlock = customerContext || ''`

### Task 2: Replace "customer" with "driver" in PWA pages
**Commit (racecontrol):** `e20d4e5`

- `register/page.tsx`: "Guardian name is required for customers under 18" → "drivers under 18"
- `dashboard/page.tsx`: "Welcome back" → "Welcome back, Driver"

## Verification Results

| Check | Expected | Result |
|-------|----------|--------|
| WA systemPrompt customer occurrences | Only customerContext code lines | PASS (2 lines, both code) |
| WA businessKnowledge customer occurrences | 0 | PASS |
| DC systemPrompt customer occurrences | 0 | PASS |
| PWA register "customers under 18" | Not found | PASS |
| PWA dashboard "Welcome back, Driver" | Found | PASS |
| WA systemPrompt syntax | No errors | PASS |
| DC systemPrompt syntax | No errors | PASS |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing] Extra user-facing "Customer" lines in WA systemPrompt.js BOOKING NOTES**
- **Found during:** Task 1
- **Issue:** Lines 156-157 had "Customer can type a number" and "Customer can say 'cancel'" — user-facing booking flow instructions not in the plan's explicit list of 14
- **Fix:** Replaced with "Driver can type..." and "Driver can say..."
- **Files modified:** `/root/racingpoint-whatsapp-bot/src/prompts/systemPrompt.js`
- **Commit:** `de7c836`

**2. [Rule 2 - Missing] Extra user-facing "Customers" in businessKnowledge.js booking section**
- **Found during:** Task 1
- **Issue:** Line 105 "Customers can book through this chat" — user-facing booking instruction not in the plan's explicit list of 2
- **Fix:** Replaced with "Drivers can book through this chat"
- **Files modified:** `/root/racingpoint-whatsapp-bot/src/prompts/businessKnowledge.js`
- **Commit:** `de7c836`

## Commits

| Repo | Hash | Message |
|------|------|---------|
| racingpoint-whatsapp-bot | `de7c836` | feat(93-02): replace "customer" with "driver" in WhatsApp bot prompts |
| racingpoint-discord-bot | `f3e0d32` | feat(93-02): replace "customer" with "driver" in Discord bot prompts |
| racecontrol | `e20d4e5` | feat(93-02): replace "customer" with "driver" in PWA register and dashboard |

## Self-Check: PASSED

| Item | Status |
|------|--------|
| SUMMARY.md exists | FOUND |
| whatsapp-bot commit de7c836 | FOUND |
| discord-bot commit f3e0d32 | FOUND |
| racecontrol commit e20d4e5 | FOUND |
