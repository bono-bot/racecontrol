# Page: Billing History
**App:** web  
**URL:** http://192.168.31.23:3200/billing/history  
**Auth:** Required

## Expected Layout
- Left sidebar navigation
- Main content area with a transaction history table
- Table columns: date, customer, pod, duration, amount, status
- Pagination or scroll for historical records
- Possible date range filter controls

## Expected Data
- Historical billing transactions (may be empty on fresh DB)
- Each row: transaction date, customer info, pod used, session duration, amount charged, payment status
- **Dynamic content (ignore for layout comparison):** `.transaction-date`, `.session-date`, timestamps, relative times

## Key Interactions
- Date range filter to narrow results
- Sortable table columns
- Clickable rows for transaction detail
- Possible export/download functionality

## What "Wrong" Looks Like
- Empty table with no headers (API failure vs. genuinely empty history)
- Unstyled HTML (static files 404)
- Dates showing in wrong timezone or raw UTC
- Missing pagination controls on large datasets
- Login redirect instead of history page
- Error banner indicating database connection failure
