# Page: Drivers
**App:** web  
**URL:** http://192.168.31.23:3200/drivers  
**Auth:** Required

## Expected Layout
- Left sidebar navigation
- Main content area with driver list (table or card grid)
- Each driver entry: name, phone, last visit, total sessions, rating/stats
- Search/filter bar at top

## Expected Data
- List of registered drivers/customers
- Per-driver info: name, contact, visit history, session count
- **Dynamic content (ignore for layout comparison):** `.last-visit`, `.total-sessions`, timestamps, relative times

## Key Interactions
- Search by name or phone number
- Clickable driver entries for profile detail
- Add new driver button
- Sort by name, last visit, total sessions

## What "Wrong" Looks Like
- Empty driver list when drivers exist in DB (API failure)
- Unstyled HTML (static files 404)
- Missing search/filter controls
- Login redirect instead of drivers page
- Error banner or database connection error
- All "last visit" fields showing same date (stale data)
