# Page: Kiosk Control
**App:** kiosk  
**URL:** http://192.168.31.23:3300/kiosk/control  
**Auth:** Required

## Expected Layout
- Control room style layout with pod grid
- Each pod represented as a card or tile with status and controls
- Game launch/stop controls per pod
- Session info panel showing current activity per pod

## Expected Data
- 8 pod tiles/cards with real-time status
- Per-pod: current game, session time, customer name, pod health
- Game selection dropdown or modal per pod
- **Dynamic content (ignore for layout comparison):** `.pod-status`, `.session-info`, timestamps, relative times

## Key Interactions
- Launch game on specific pod
- Stop/kill game on specific pod
- View pod details (click pod tile)
- Assign customer to pod
- Emergency stop all

## What "Wrong" Looks Like
- Empty pod grid (0 pods shown -- API returning empty)
- Unstyled HTML (static files 404)
- All pods showing "offline" when venue is open
- Game launch buttons unresponsive (WebSocket disconnect)
- Redirect to kiosk landing (auth issue)
- Error loading control panel data
- Session info showing stale/frozen data
