# Page: Fleet
**App:** web  
**URL:** http://192.168.31.23:3200/fleet  
**Auth:** Required

## Expected Layout
- Left sidebar navigation (same as all web pages)
- Main content area with a grid or table of pod cards/rows
- Each pod entry shows: pod number, status, build ID, uptime, last seen, WS latency

## Expected Data
- 8 pod entries (Pod 1 through Pod 8)
- Each pod shows online/offline status with color indicator
- Build ID hash displayed per pod
- Uptime and last-seen timestamps per pod
- **Dynamic content (ignore for layout comparison):** `.pod-uptime`, `.last-seen`, `.ws-latency`, `.build-id`, timestamps, relative times

## Key Interactions
- Pod entries may be clickable to view pod detail
- Possible filter/search for specific pods
- Refresh or real-time update of pod statuses

## What "Wrong" Looks Like
- 0 pods shown (empty fleet -- server API returning empty array)
- All pods showing "offline" when venue is open
- Missing build ID or uptime columns (API schema mismatch)
- Unstyled HTML (static files 404)
- Login redirect instead of fleet page
- Error banner or crash screen
