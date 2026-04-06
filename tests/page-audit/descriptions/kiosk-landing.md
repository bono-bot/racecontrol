# Page: Kiosk Landing
**App:** kiosk  
**URL:** http://192.168.31.23:3300/kiosk  
**Auth:** None

## Expected Layout
- Full-screen landing page (no sidebar -- kiosk mode)
- Racing Point branding with logo
- Welcome/greeting message
- Clock or time display
- Entry point buttons for customers and staff

## Expected Data
- Current time/clock display
- Greeting appropriate to time of day (morning/afternoon/evening)
- **Dynamic content (ignore for layout comparison):** `.clock`, `.greeting-time`, timestamps

## Key Interactions
- Customer entry button (leads to registration/booking)
- Staff login button (leads to staff PIN entry)
- Touch-friendly large buttons for kiosk use

## What "Wrong" Looks Like
- Blank white or black screen (app not loaded)
- Unstyled HTML with no CSS (static files 404)
- Missing Racing Point branding/logo
- Clock showing wrong time or frozen
- No visible entry buttons
- Error screen or unhandled exception
- Redirect to login page (should be public)
