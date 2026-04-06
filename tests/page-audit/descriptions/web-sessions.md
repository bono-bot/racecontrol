# Page: Sessions
**App:** web  
**URL:** http://192.168.31.23:3200/sessions  
**Auth:** Required

## Expected Layout
- Left sidebar navigation
- Main content area with session list (table or cards)
- Each session entry shows: customer, pod, game, start time, duration, status
- Status indicators (active, completed, cancelled)

## Expected Data
- List of gaming sessions (active and recent)
- Session details: customer, assigned pod, game title, start time, elapsed duration
- Status badges (active/completed/cancelled)
- **Dynamic content (ignore for layout comparison):** `.session-duration`, `.session-start`, timestamps, relative times

## Key Interactions
- Filter by status (active, completed, all)
- Clickable session rows for detail view
- Possible session management actions (end session, extend)

## What "Wrong" Looks Like
- Blank page or empty table with no column headers
- Unstyled HTML (static files 404)
- All sessions showing "active" status (stale data, no WS updates)
- Duration timers frozen (WebSocket disconnect)
- Login redirect instead of sessions page
- Error screen or API timeout message
