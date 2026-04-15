# messageHandler.js — James Venue Closure Patch (2026-04-14)

This patch adds the venue closure intercept to `src/services/messageHandler.js`.
The full `messageHandler.FULL-DIFF.patch` file in this directory includes ~500 lines
of Bono's pre-existing WIP. This file shows ONLY James's venue-closure additions.

## Change 1 — Add import

Location: top of file, next to existing `istTimeService` import.

```javascript
// ADD this line after: const { isVenueOpen, getVenueStatusMessage } = require('./istTimeService');
const { isVenueClosed, getClosureMessage, getClosureContextForPrompt } = require('./venueStatusService');
```

## Change 2 — Early intercept (the critical one)

Location: inside `processMessage()`, immediately AFTER the "Blocked user message ignored"
block and BEFORE the spam analysis block.

```javascript
// EXISTING code (do not change):
      logger.info({ remoteJid }, 'Blocked user message ignored');
      return;
    }

// ADD the block below right here:
    // ── Venue closure intercept — before ANY AI/intelligence processing ──
    if (isVenueClosed() && !googleCommandHandler.isAdmin(remoteJid)) {
      const closureMsg = getClosureMessage();
      conversationService.saveMessage(remoteJid, 'user', text);
      conversationService.saveMessage(remoteJid, 'assistant', closureMsg);
      await evolutionService.sendText(remoteJid, closureMsg);
      await evolutionService.sendPresence(remoteJid, 'paused');
      logger.info({ remoteJid, pushName, reason: 'venue_closed' }, 'Venue closure message sent');
      return;
    }

// EXISTING code continues:
    // Spam analysis — accumulates score, auto-blocks at threshold
```

## Change 3 — System prompt safety net

Location: where `systemPrompt = buildSystemPrompt(...)` is called for non-admin users.

```javascript
// BEFORE:
      systemPrompt = buildSystemPrompt(contextBlock + '\n' + intelligenceContext);

// AFTER:
      const closureCtx = getClosureContextForPrompt();
      systemPrompt = buildSystemPrompt(contextBlock + '\n' + intelligenceContext + closureCtx);
```

## Why these changes

A customer called reporting the bot told them RacingPoint was open, but the venue was
closed because Race Control was down. The bot had NO concept of temporary closure —
`istTimeService.isVenueOpen()` only checked the clock, and `businessKnowledge.js` hardcoded
"open all days". The intercept is placed as early as possible (before AI processing)
to save tokens and guarantee the closure message is sent regardless of what Claude Haiku
would generate. The system prompt injection is a safety net if the intercept is ever bypassed.

## Activation / Deactivation (no code change needed)

- Close: `touch /root/racingpoint-whatsapp-bot/VENUE_CLOSED`
- Reopen: `rm /root/racingpoint-whatsapp-bot/VENUE_CLOSED`
- Custom message: `echo "Back Tuesday 10am" > /root/racingpoint-whatsapp-bot/VENUE_CLOSED_MESSAGE`
- Auto-close: 3+ consecutive rcCacheService failures (15 min) sets closed automatically

No pm2 restart needed after flag toggle — state is checked per incoming message.
