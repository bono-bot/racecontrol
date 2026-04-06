# Page: Home
**App:** web  
**URL:** http://192.168.31.23:3200/  
**Auth:** Required

## Expected Layout
- Left sidebar navigation with links to all major sections (Fleet, Billing, Sessions, Drivers, Games, etc.)
- Top header bar with page title and WebSocket connection status indicator
- Main content area with summary cards (pod status, active sessions, revenue)
- Dashboard grid layout with key metrics

## Expected Data
- Pod status cards showing 8 pods with online/offline indicators
- Active sessions count (numeric, changes in real-time)
- Revenue today figure (currency, changes in real-time)
- **Dynamic content (ignore for layout comparison):** `.ws-status`, `.pod-status-indicator`, `.active-sessions-count`, `.revenue-today`, `[data-live="true"]`, timestamps, relative times

## Key Interactions
- Sidebar links navigate to respective sections
- Pod status cards may be clickable for details
- WebSocket status indicator shows connection state

## What "Wrong" Looks Like
- Blank page with no content (server down or auth redirect loop)
- Unstyled HTML with no CSS/JS (static files 404 -- wrong outputFileTracingRoot)
- Login redirect loop (middleware protecting login page)
- All pods showing offline/0 pods (API returning empty arrays -- server down)
- Red dot or "Connecting..." text (WebSocket disconnect)
- Error banner or exception screen
- Missing sidebar navigation
