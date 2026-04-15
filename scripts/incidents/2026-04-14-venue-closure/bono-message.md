P0 INCIDENT + VENUE CLOSURE PATCH — James 2026-04-14 ~21:00 IST

## What happened
A customer called saying the WhatsApp bot told them RacingPoint was OPEN while the venue was actually CLOSED (Race Control down, Uday's P0 laps issue). Root cause in the bot: `istTimeService.isVenueOpen()` only checked the clock, and `businessKnowledge.js` hardcoded "open all days" — there was zero mechanism for temporary closure.

## What I patched (live on the VPS right now)

New file: `src/services/venueStatusService.js`
Patched: `src/services/istTimeService.js`, `src/services/rcCacheService.js`, `src/services/messageHandler.js`

Closure sources (any one = closed):
1. `VENUE_CLOSED` env var (truthy)
2. `/root/racingpoint-whatsapp-bot/VENUE_CLOSED` flag file — **currently exists, venue marked closed**
3. Auto-detect: 3+ consecutive `rcCacheService` failures (15+ min of racecontrol down) → auto-closed

Intercept is placed in `processMessage()` right after the blocked-user check and BEFORE any AI/intelligence call. When closed, non-admin messages get a closure notice with zero Claude API cost. Also injects a safety-net closure note into `buildSystemPrompt()` for non-admin flows.

Verification: simulated inbound via `handleMessage()` with mocked `evolutionService.sendText` — only the closure message was captured, `reason: venue_closed` logged, zero AI calls.

## The partner-boundary problem — WHY I DID NOT COMMIT

Your `/root/racingpoint-whatsapp-bot` repo is in a major WIP state:
- 5 commits ahead of `origin/master` already
- 13 files modified (uncommitted) including `messageHandler.js` with ~500 lines of your unfinished work (flow handlers, cafe service, intelligence service, referral service, lap time card service, spam guard, etc.)
- 22 untracked files — `istTimeService.js`, `rcCacheService.js`, `cafeService.js`, `flowHandlers.js`, `customerProfileService.js`, `intelligenceService.js`, `followUpService.js`, `referralService.js`, `lapTimeCardService.js`, `templateCampaignService.js`, `vpsBookingService.js`, `spamGuard.js`, `staffAlertService.js`, `analyticsService.js`, and more

If I had committed my 4 files, I'd have been committing your files under my authorship, or sweeping ~500 lines of your WIP into a "snapshot" commit. Both are boundary violations — you may have intentional reasons nothing is committed yet.

**Please: commit your bot WIP when ready, and include the venue-closure patch at the same time.**

## Where the patch is backed up

`/root/venue-closure-backup-2026-04-14/` on the VPS:
- `venueStatusService.js` — new file, copy as-is to `src/services/venueStatusService.js`
- `istTimeService.js` — full file with venue-closure integration (my only edit: override `isVenueOpen()` + `getVenueStatusMessage()` when closed)
- `rcCacheService.js` — full file with venue-closure health reporting (my only edit: call `reportRcHealth(podsOk)` after each refresh)
- `messageHandler.js.FULL-DIFF.patch` — full 44KB diff vs HEAD (contains your WIP + my 15-line intercept, provided as reference)
- `messageHandler-JAMES-PATCH-ONLY.md` — documentation of just my 3 changes to messageHandler.js so you can apply them cleanly on top of your final WIP
- `apply-venue-closure.sh` — original install script (python3-based in-place patcher)

## Activation / deactivation (no restart needed)

- Close: `touch /root/racingpoint-whatsapp-bot/VENUE_CLOSED`
- Reopen: `rm /root/racingpoint-whatsapp-bot/VENUE_CLOSED`
- Custom message: `echo "Back Tuesday 10am" > /root/racingpoint-whatsapp-bot/VENUE_CLOSED_MESSAGE`
- State is read per-message, so no pm2 restart needed after toggle

## Action items for you

1. Incorporate the 4 files into your next commit of the bot WIP (instructions in the backup dir)
2. Review the closure message text — see default in `venueStatusService.js` `DEFAULT_CLOSURE_MESSAGE`. Happy to adjust wording based on what Uday wants customers to see.
3. Consider whether to also patch the second `whatsapp-bot` pm2 process (PID 19, port 3150, likely the staff bot on 7075778180). I only patched `racingpoint-bot`. If they share code, a restart of that one picks up the patch; if they're separate codebases, it needs its own patch.
4. When Uday/James fix Race Control and the venue reopens, remember to `rm VENUE_CLOSED`.

## NOT tested

- Admin bypass path (`googleCommandHandler.isAdmin(remoteJid)`) — intercept should skip admins, logic is there but not exercised
- Auto-close via 3x `rcCacheService` failure — would need racecontrol to actually be unreachable for 15+ min to trigger
- Real WhatsApp round trip — the verification mocked `evolutionService.sendText`, never sent an actual message through Evolution API
- `whatsapp-bot` (second pm2 process) — only patched `racingpoint-bot`
- First-message sizzle greeting path for a brand-new customer — intercept runs before sizzle, should preempt, but not verified with a real first-message customer
- Custom message via `VENUE_CLOSED_MESSAGE` file — code path exists, file not created during test

## Session metrics
Claims: 2 | Corrections: 0 | FCR: 0% | G9s: 0
