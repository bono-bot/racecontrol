# Page: Billing
**App:** web  
**URL:** http://192.168.31.23:3200/billing  
**Auth:** Required

## Expected Layout
- Left sidebar navigation
- Main content area showing active billing sessions
- Table or card layout with session details (customer, pod, duration, amount)
- Action buttons for starting/stopping billing sessions

## Expected Data
- List of active billing sessions (may be empty if no sessions running)
- Each session shows: customer name/phone, assigned pod, elapsed time, running cost
- Session timer counting up in real-time
- Wallet balance display for active customers
- **Dynamic content (ignore for layout comparison):** `.session-timer`, `.active-billing`, `.wallet-balance`, timestamps, relative times

## Key Interactions
- Start new billing session button
- Stop/end session buttons per row
- Customer selection/search
- Pod assignment controls

## What "Wrong" Looks Like
- Blank content area with no table/cards structure
- Unstyled HTML (static files 404)
- Session timers frozen (WebSocket disconnect)
- Error loading billing data (API failure)
- Login redirect instead of billing page
- Wallet balances showing 0 for all customers
