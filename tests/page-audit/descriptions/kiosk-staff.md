# Page: Kiosk Staff
**App:** kiosk  
**URL:** http://192.168.31.23:3300/kiosk/staff  
**Auth:** Required

## Expected Layout
- Staff dashboard layout (different from customer kiosk view)
- Pod management controls visible
- Active session overview
- Quick-action buttons for common staff tasks

## Expected Data
- Pod status overview (8 pods with status indicators)
- Active sessions count and details
- Staff clock/time display
- **Dynamic content (ignore for layout comparison):** `.staff-clock`, `.active-count`, timestamps, relative times

## Key Interactions
- Pod control buttons (start/stop/assign)
- Session management (start billing, end session)
- Navigation to other staff functions (control, register)
- Quick action shortcuts

## What "Wrong" Looks Like
- Redirect to kiosk landing page (auth middleware blocking login page -- known failure mode)
- Staff page looking identical to customer landing page (redirect loop)
- Unstyled HTML (static files 404)
- No pod controls visible
- Blank content area
- Error banner or API failure message
- PIN entry modal that never submits (JS not loaded)
