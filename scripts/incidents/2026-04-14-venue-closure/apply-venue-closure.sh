#!/bin/bash
# Apply venue closure feature to WhatsApp bot
# Run on Bono VPS: bash /tmp/apply-venue-closure.sh
set -e

BOT_DIR="/root/racingpoint-whatsapp-bot"
SVC_DIR="$BOT_DIR/src/services"

echo "=== Applying venue closure feature ==="

# 1. Copy venueStatusService.js
echo "[1/4] Installing venueStatusService.js..."
cp /tmp/venueStatusService.js "$SVC_DIR/venueStatusService.js"

# 2. Patch istTimeService.js — add venue closure override to isVenueOpen()
echo "[2/4] Patching istTimeService.js..."
cd "$BOT_DIR"

# Add import at top and override isVenueOpen
python3 -c "
import re

with open('$SVC_DIR/istTimeService.js', 'r') as f:
    content = f.read()

# Add venue status import after the constants
insert_after = \"const VENUE_CLOSE_HOUR = 24; // midnight (hour 0 next day)\"
import_line = \"\\n// Venue closure override\\nconst { isVenueClosed, getClosureMessage } = require('./venueStatusService');\"
content = content.replace(insert_after, insert_after + import_line)

# Override isVenueOpen to check closure first
old_func = '''function isVenueOpen() {
  const ist = getCurrentIST();
  const hour = ist.getHours();
  return hour >= VENUE_OPEN_HOUR; // hours 12-23 = open
}'''

new_func = '''function isVenueOpen() {
  // Venue closure override — manual or auto-detected
  if (isVenueClosed()) return false;
  const ist = getCurrentIST();
  const hour = ist.getHours();
  return hour >= VENUE_OPEN_HOUR; // hours 12-23 = open
}'''

content = content.replace(old_func, new_func)

# Override getVenueStatusMessage to return closure message when closed
old_status = '''function getVenueStatusMessage() {
  const ist = getCurrentIST();
  const hour = ist.getHours();'''

new_status = '''function getVenueStatusMessage() {
  // Return closure message if venue is closed (manual or auto)
  if (isVenueClosed()) return getClosureMessage();
  const ist = getCurrentIST();
  const hour = ist.getHours();'''

content = content.replace(old_status, new_status)

with open('$SVC_DIR/istTimeService.js', 'w') as f:
    f.write(content)

print('  istTimeService.js patched OK')
"

# 3. Patch rcCacheService.js — report RC health to venueStatusService
echo "[3/4] Patching rcCacheService.js..."
python3 -c "
with open('$SVC_DIR/rcCacheService.js', 'r') as f:
    content = f.read()

# Add import at top (after existing requires)
old_require = \"const RC_API_URL = config.racecontrol.apiUrl;\"
new_require = \"const { reportRcHealth } = require('./venueStatusService');\\nconst RC_API_URL = config.racecontrol.apiUrl;\"
content = content.replace(old_require, new_require)

# Add health reporting at end of refreshCache
old_log = '''  logger.info(
    {
      pods: !!cache.pods.data,
      pricing: !!cache.pricing.data,
      bookings: cache.bookingCount.data,
    },
    'RC cache refreshed'
  );
}'''

new_log = '''  // Report health to venue status service for auto-close detection
  const podsOk = !!cache.pods.data && !cache.pods.error;
  reportRcHealth(podsOk);

  logger.info(
    {
      pods: !!cache.pods.data,
      pricing: !!cache.pricing.data,
      bookings: cache.bookingCount.data,
    },
    'RC cache refreshed'
  );
}'''

content = content.replace(old_log, new_log)

with open('$SVC_DIR/rcCacheService.js', 'w') as f:
    f.write(content)

print('  rcCacheService.js patched OK')
"

# 4. Patch messageHandler.js — add early venue closure intercept + inject into system prompt
echo "[4/4] Patching messageHandler.js..."
python3 -c "
with open('$SVC_DIR/messageHandler.js', 'r') as f:
    content = f.read()

# Add import after existing istTimeService import
old_import = \"const { isVenueOpen, getVenueStatusMessage } = require('./istTimeService');\"
new_import = old_import + \"\\nconst { isVenueClosed, getClosureMessage, getClosureContextForPrompt } = require('./venueStatusService');\"
content = content.replace(old_import, new_import)

# Add venue closure intercept after blocked user check
# Find the 'Blocked user message ignored' log line and its return statement
old_blocked = '''      logger.info({ remoteJid }, 'Blocked user message ignored');
      return;
    }

    // Spam analysis'''

new_blocked = '''      logger.info({ remoteJid }, 'Blocked user message ignored');
      return;
    }

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

    // Spam analysis'''

content = content.replace(old_blocked, new_blocked)

# Also inject closure context into the system prompt builder (safety net for if intercept is bypassed)
old_prompt = '''      systemPrompt = buildSystemPrompt(contextBlock + '\\n' + intelligenceContext);'''
new_prompt = '''      const closureCtx = getClosureContextForPrompt();
      systemPrompt = buildSystemPrompt(contextBlock + '\\n' + intelligenceContext + closureCtx);'''
content = content.replace(old_prompt, new_prompt)

with open('$SVC_DIR/messageHandler.js', 'w') as f:
    f.write(content)

print('  messageHandler.js patched OK')
"

echo ""
echo "=== All patches applied ==="
echo "To activate closure NOW: touch $BOT_DIR/VENUE_CLOSED"
echo "To deactivate: rm $BOT_DIR/VENUE_CLOSED"
echo "Custom message: echo 'your message' > $BOT_DIR/VENUE_CLOSED_MESSAGE"
echo ""
echo "Restart bot: pm2 restart racingpoint-bot"
