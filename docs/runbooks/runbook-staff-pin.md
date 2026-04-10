# Change Staff PIN — Racing Point
## When to use this: A staff member forgot their PIN, or their PIN needs to change

## Step by step:
1. Open Admin Staff page → `http://192.168.31.23:3201/staff/manage` ⚠️ [requires Phase 347 deploy]
2. Find the staff member in the list → click **Change PIN**
3. Enter the new 4-digit numeric PIN → enter it again to confirm
4. Click **Save** → wait for the green **"Verified on venue"** confirmation message
5. Ask the staff member to test their new PIN at the POS immediately

## If stuck: WhatsApp Bono — describe which staff member and what error appears

## DO NOT:
- **DO NOT** use sqlite3, any script, or Command Prompt to change PINs
- **DO NOT** ask James to SSH in — the admin page handles this completely
- **DO NOT** share PINs over WhatsApp — only enter them directly on the Admin page

---
*Last updated: 2026-04-11 | Racing Point eSports*
