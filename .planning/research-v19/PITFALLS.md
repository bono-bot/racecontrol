# Domain Pitfalls: Cafe Inventory, Ordering & Marketing

**Domain:** Cafe operations bolted onto an existing gaming center platform with shared wallet
**Researched:** 2026-03-22
**Confidence:** HIGH (most pitfalls are well-documented in POS/inventory literature; some are specific to this codebase based on CLAUDE.md standing rules)

---

## Critical Pitfalls

Mistakes that cause data corruption, lost revenue, or require architectural rewrites.

### Pitfall 1: Race Condition on Stock Decrement (Double-Sell)

**What goes wrong:** Two simultaneous orders (one from PWA, one from POS) both read stock=1 for the same item, both proceed, both decrement. Result: stock goes to -1 and a customer gets an item that does not exist. This is the most common concurrency bug in ordering systems.

**Why it happens:** Naive check-then-decrement pattern: `if stock > 0 { stock -= 1 }`. With concurrent requests this is a textbook TOCTOU (time-of-check, time-of-use) race.

**Consequences:** Overselling inventory, customer frustration, staff confusion when receipt prints for an item that is gone, and negative stock counts that break downstream reporting.

**Prevention:**
- Use atomic database operations: `UPDATE items SET stock = stock - 1 WHERE id = ? AND stock > 0` and check rows-affected. If 0 rows affected, the item is out of stock — reject the order.
- Never do a separate SELECT followed by UPDATE. The decrement and the availability check must be a single atomic operation.
- If using server-side JSON files (per project constraints), use a Mutex or RwLock around the entire read-modify-write cycle. JSON files have no built-in atomicity.
- For the Rust/Axum backend: wrap stock state in `Arc<Mutex<...>>` or use `tokio::sync::Mutex` for the async context. Keep the critical section short (just the decrement, not the entire order processing).

**Detection:** Stock values going negative in any report. Orders completing for items with stock=0. Two receipts printing simultaneously for the last unit of an item.

**Which phase should address it:** The very first phase that implements stock auto-decrement (Ordering & Billing). Must be correct from day one — retrofitting atomicity is painful.

---

### Pitfall 2: Shared Wallet Double-Deduction or Insufficient-Balance Race

**What goes wrong:** A customer has Rs 1000 in their RP wallet. They start a gaming session (Rs 900) and simultaneously order food via PWA (Rs 200). Both transactions read balance=1000, both proceed, wallet goes to -100. Alternatively, a deduction fails halfway and money is deducted but the order is not created (or vice versa).

**Why it happens:** The wallet is shared across gaming billing and cafe ordering — two completely separate code paths that both mutate the same balance. The existing gaming billing was never designed to contend with a second deduction source.

**Consequences:** Negative wallet balances, disputed charges, customers able to spend more than they have, or the opposite: money deducted but no order placed (lost revenue and angry customer).

**Prevention:**
- All wallet operations must go through a single transactional function (not duplicated in cafe and gaming code separately). If one already exists for gaming, cafe orders must call the same function.
- Use optimistic concurrency: `UPDATE wallets SET balance = balance - ? WHERE id = ? AND balance >= ?` as a single atomic operation. Check rows-affected.
- Wrap order-creation + wallet-deduction in a transaction. If order creation fails after wallet deduction, roll back the deduction. If wallet deduction fails, do not create the order.
- Consider a brief per-customer lock during checkout to prevent concurrent purchases from the same wallet.

**Detection:** Wallet balances going negative. Orders without corresponding wallet transactions. Wallet transactions without corresponding orders. Customer complaints about being charged without receiving an order.

**Which phase should address it:** Ordering & Billing phase. The wallet integration is the highest-risk piece of the entire project because it touches existing production money flows.

---

### Pitfall 3: PDF/Spreadsheet Import Creates Garbage Data

**What goes wrong:** The PDF menu has inconsistent formatting — item names in different cases, prices as "Rs 150" vs "150" vs "150.00", categories spelled differently ("Beverages" vs "beverages" vs "Drinks"), cost prices missing for some items. The import silently creates items with wrong prices, missing data, or duplicate entries.

**Why it happens:** PDFs are not structured data. Even well-formatted PDFs parse differently depending on the library. Spreadsheets from non-technical staff often have merged cells, inconsistent column usage, and no data validation.

**Consequences:** Wrong prices in POS (charging customers Rs 15 instead of Rs 150 because the parser dropped a zero). Duplicate items cluttering the menu. Missing cost prices making profit calculations meaningless.

**Prevention:**
- Never auto-publish imported items. Import into a "draft" or "review" state. Admin must review and confirm each batch before items go live.
- Show a preview table after parsing, before committing to the database. Highlight any rows where parsing confidence is low (missing fields, unusual price formats).
- Validate hard constraints: price must be > 0, name must be non-empty, category must match a known set (or flag as "uncategorized" for review).
- Normalize currency strings: strip "Rs", "INR", commas, and whitespace before parsing the number.
- For PDF parsing, prefer requesting the source spreadsheet from the owner. Only fall back to PDF parsing if the spreadsheet truly does not exist.

**Detection:** Items appearing with Rs 0 price, empty names, or "undefined" category. Duplicate item names in different categories. Cost price higher than selling price (common parse error where columns shifted).

**Which phase should address it:** Menu & Item Management phase (the import feature specifically). Build the preview+confirm flow before building the parser — the UX prevents damage regardless of parser quality.

---

### Pitfall 4: Promo Stacking Destroys Margins

**What goes wrong:** A combo deal (Burger + Coke = Rs 149), a happy hour discount (20% off 3-6 PM), and a gaming+cafe bundle all apply to the same order simultaneously. The customer pays Rs 80 for items that cost Rs 120 to make. No one notices until end-of-month accounting.

**Why it happens:** Each promo is built independently. The combo system does not know about happy hour. The gaming bundle does not know about cafe combos. Without explicit stacking rules, every discount applies.

**Consequences:** Selling below cost price. Margin erosion that is invisible until financial review. Customers discovering exploits and sharing them. Staff unable to explain why a bill looks wrong.

**Prevention:**
- Define stacking rules up front: "Only one promotion applies per item" or "Combos and happy hours do not stack" or explicit priority (combo overrides happy hour).
- Calculate the final price server-side, never client-side. The POS/PWA displays the final price but does not compute it.
- Always calculate and log the effective margin for each order line. Alert (or block) if any order line sells below cost price.
- Store the applied promotion ID with each order line item, so auditing is trivial.
- Default to "most specific promotion wins" — a named combo beats a blanket percentage discount.

**Detection:** Orders where total paid < total cost price. Multiple promotion IDs on a single order line. Customer-reported prices that staff cannot reproduce.

**Which phase should address it:** Promotions & Deals phase. The stacking rules must be designed before the first promotion type is implemented, not bolted on after three different promo types exist.

---

## Moderate Pitfalls

### Pitfall 5: Thermal Receipt Printing from a Web App Is Unreliable

**What goes wrong:** The web-based POS cannot directly access the USB thermal printer. Printing works in development (localhost), fails in production (network). Printing blocks the UI thread. Receipts come out garbled (wrong encoding, ESC/POS commands not supported by the specific printer model).

**Why it happens:** Browsers sandbox hardware access. ESC/POS is a low-level protocol that varies between printer manufacturers. The existing POS already prints receipts — but the cafe order flow is a new code path that may not reuse the same printing infrastructure.

**Prevention:**
- Do not build a new printing path. Reuse the existing POS receipt printing infrastructure. Cafe orders should produce receipt data in the same format the existing system already knows how to print.
- If the existing POS uses a local print agent or WebSocket bridge, route cafe receipts through the same bridge.
- If the existing POS uses `window.print()` with CSS formatting, continue that pattern. Do not introduce ESC/POS raw commands unless the existing system already uses them.
- Test with the actual printer model on-site (not a PDF print preview).
- Always have a "reprint receipt" button — printers jam, paper runs out, connection drops.

**Detection:** Staff reporting "receipt did not print" after an order. Garbled characters on receipts. Print commands silently failing with no error shown to the user.

**Which phase should address it:** Ordering & Billing phase. Validate the printing path early with a test receipt before building the full order flow.

---

### Pitfall 6: Low-Stock Alerts Become Alert Fatigue

**What goes wrong:** 40 items each have a low-stock threshold. On a busy day, 15 items cross the threshold within an hour. Staff receive 15 WhatsApp messages, 15 emails, and 15 dashboard banners. They start ignoring all alerts. A genuinely critical out-of-stock (water bottles, the most-ordered item) gets buried.

**Why it happens:** Treating all items equally. No throttling. No grouping. Three channels multiplied by many items = noise.

**Prevention:**
- Group low-stock alerts into a single digest. Instead of one message per item, send "5 items are low on stock: [list]" once per 30 minutes (or configurable interval).
- Differentiate severity: "out of stock" (zero remaining) gets an immediate alert on all channels. "Low stock" (below threshold but not zero) gets a batched digest.
- Allow per-item channel configuration: water bottles = WhatsApp + dashboard (urgent). Garnishes = dashboard only (can wait).
- Include a "restock" action link/button in the dashboard alert, so staff can acknowledge and record the restock in one click.
- Rate-limit WhatsApp alerts to avoid hitting WhatsApp API limits (see Pitfall 8).

**Detection:** Staff reporting they "did not see" a low-stock alert that was sent. Alert volume exceeding 20+ per day. Staff muting WhatsApp notifications from the bot number.

**Which phase should address it:** Inventory Tracking phase, when implementing the alert system. Design the digest/throttle before connecting the three channels.

---

### Pitfall 7: Inventory Model Does Not Fit Cafe Reality

**What goes wrong:** The system tracks stock as simple per-item counts (10 burgers, 20 cokes). But in reality: burgers are made from buns (counted) + patties (counted) + sauce (uncountable bulk). The system shows "10 burgers in stock" but there are only 3 buns left. Or: chai is unlimited (made from bulk tea leaves) but gets a stock count of 0 when no one set it up.

**Why it happens:** Building a full recipe/ingredient decomposition system (item = sum of ingredients) when the project scope explicitly excludes it. But also failing to categorize items correctly: some items are countable (water bottles, packaged snacks), some are uncountable (chai from a pot, sauces).

**Consequences:** Stock counts that do not reflect reality. Staff losing trust in the system and reverting to manual tracking. Items showing "out of stock" that are actually available (or vice versa).

**Prevention:**
- Categorize items into two types at the data model level: **countable** (discrete units, auto-decrement works) and **uncountable** (bulk/made-to-order, admin manually marks available/unavailable).
- Do NOT build ingredient-level decomposition. The project scope correctly excludes this. But do make the distinction between countable and uncountable explicit in the UI.
- For countable items: auto-decrement on sale, low-stock alerts, restock flow.
- For uncountable items: just an on/off toggle. No stock number, no auto-decrement, no alerts. Staff flips "Chai" to unavailable when the pot is empty.
- Document this distinction clearly for staff — it is the most common source of confusion.

**Detection:** Staff repeatedly setting stock to "999" for items they consider unlimited. Items with stock=0 that are still being served. Staff ignoring the system for certain item categories.

**Which phase should address it:** Menu & Item Management phase (data model design). The countable vs. uncountable distinction must be in the schema from the start, not added later.

---

### Pitfall 8: WhatsApp Marketing Broadcast Gets the Bot Banned

**What goes wrong:** The marketing feature sends promo blasts to the entire customer list. WhatsApp detects this as spam (especially if customers have not opted in). The bot number gets throttled or banned. Existing operational WhatsApp flows (session alerts, low-stock notifications, staff comms) stop working because they share the same number.

**Why it happens:** WhatsApp Business API has strict rules: marketing messages require opt-in, templates must be approved, and new senders start at 1,000 unique users/day. The unofficial WhatsApp bot (which Racing Point likely uses given the comms-link setup) has even stricter implicit limits — bulk sending gets flagged fast.

**Consequences:** Loss of the WhatsApp bot for ALL purposes (not just marketing). Operational alerts stop. Customer communication breaks. Number may be permanently banned.

**Prevention:**
- Never share the operational bot number with marketing broadcasts. Use a separate number for marketing, or do not use WhatsApp for bulk promos at all.
- If using WhatsApp Business API (official): register marketing templates with Meta, get them approved, respect tier limits (start at 1K/day), only message opted-in customers.
- If using the unofficial bot: do NOT send bulk marketing. Use it only for transactional/operational messages (order confirmations, low-stock alerts). Use email or in-app notifications for marketing.
- Implement opt-in/opt-out tracking. Only send to customers who explicitly opted in. Respect unsubscribe requests immediately.
- Rate-limit outbound messages: max N per minute, with jitter. Never blast 500 messages in 30 seconds.

**Detection:** WhatsApp message delivery failures increasing. Bot number showing "temporarily banned" status. Customers reporting they stopped receiving operational messages.

**Which phase should address it:** Marketing & Content phase. But the decision about which number to use (or whether to use WhatsApp for marketing at all) must be made before implementation begins.

---

### Pitfall 9: Wallet Deduction UX Surprises Customers

**What goes wrong:** Customer loads Rs 1000 for gaming. Does not realize cafe orders also deduct from the same balance. Orders food worth Rs 300, then their gaming session ends early because balance is insufficient. Or: customer disputes a cafe charge they did not authorize (their friend ordered food on their account from the PWA).

**Why it happens:** The shared wallet is a feature for the business (one system) but a potential surprise for the customer (they mentally separate "gaming money" from "food money"). PWA self-ordering using wallet balance without a confirmation PIN means anyone who knows the customer's account can spend their money.

**Consequences:** Customer complaints. Disputes. Trust erosion. Potential chargeback-like situations where customers demand refunds.

**Prevention:**
- Show wallet balance prominently in the PWA before and after each cafe order, with clear labeling: "This will be deducted from your Racing Point wallet."
- Require explicit confirmation for each cafe order: "Confirm order: Rs 250 will be deducted from your RP wallet (current balance: Rs 800)."
- Consider a simple PIN or confirmation step for PWA orders to prevent unauthorized spending by others.
- Show a unified transaction history: gaming charges + cafe charges in one view, so the customer understands where their money went.
- Set a configurable per-order limit for cafe spending (e.g., max Rs 500 per order) to prevent accidental large deductions.

**Detection:** Customers asking "where did my balance go?" Support tickets about unauthorized cafe charges. Gaming sessions ending unexpectedly due to insufficient balance.

**Which phase should address it:** Ordering & Billing phase. The wallet deduction UX must be designed alongside the order flow, not as an afterthought.

---

## Minor Pitfalls

### Pitfall 10: Marketing Image Generation Produces Low-Quality Output

**What goes wrong:** Auto-generated promo graphics look amateur — wrong brand colors, text overflow, images sized wrong for Instagram Stories vs. Posts vs. in-store screens. Staff stops using them and makes graphics manually, making the entire feature worthless.

**Prevention:**
- Use fixed templates with variable data (item names, prices, promo text) rather than fully generative AI images.
- Design 3-4 templates per format (story, post, landscape for screens) using Racing Point brand guidelines (Racing Red #E10600, Montserrat, Enthocentric fonts).
- Validate text length before rendering — truncate or shrink font if item names are too long.
- Generate a preview before publishing. Never auto-post to social media without human approval.

**Which phase should address it:** Marketing & Content phase. Template design should happen before the generation code.

---

### Pitfall 11: Time-Based Promos and IST Timezone Bugs

**What goes wrong:** Happy hour is 3-6 PM IST. The server processes the time in UTC. Happy hour actually runs from 9:30 AM to 12:30 PM IST, or from 8:30 PM to 11:30 PM IST, depending on which direction the offset was applied.

**Prevention:**
- All promo time comparisons must use IST (UTC+5:30), consistent with the project's standing rule.
- Store promo times as naive local time (HH:MM) with an explicit timezone label, not as UTC timestamps.
- Test with times that cross midnight (e.g., a late-night deal from 10 PM to 1 AM).
- Log the effective promo window on activation so staff can verify.

**Which phase should address it:** Promotions & Deals phase.

---

### Pitfall 12: Category Naming Drift

**What goes wrong:** Admin creates "Beverages" as a category. Later adds items under "beverages" (lowercase) or "Drinks" (synonym). The menu now shows three categories that are logically the same thing. Promos targeting "Beverages" do not apply to items in "Drinks."

**Prevention:**
- Categories should be a fixed, admin-managed list — not free-text on each item.
- Item creation/edit should select from a dropdown of existing categories, with an "add new category" option that is explicit.
- Normalize category names on creation (trim whitespace, consistent casing).

**Which phase should address it:** Menu & Item Management phase (data model).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Menu & Item Management | PDF import creates bad data (Pitfall 3) | Preview + confirm flow, never auto-publish |
| Menu & Item Management | Countable vs. uncountable items conflated (Pitfall 7) | Two item types in the schema from day one |
| Menu & Item Management | Category naming drift (Pitfall 12) | Categories as a managed list, not free-text |
| Inventory Tracking | Alert fatigue from low-stock notifications (Pitfall 6) | Digest/batch alerts, severity tiers |
| Ordering & Billing | Stock decrement race condition (Pitfall 1) | Atomic decrement, single operation |
| Ordering & Billing | Shared wallet double-deduction (Pitfall 2) | Single transactional wallet function |
| Ordering & Billing | Receipt printing fails silently (Pitfall 5) | Reuse existing print path, test early |
| Ordering & Billing | Customer surprise at shared wallet (Pitfall 9) | Clear UX, confirmation step, balance display |
| Promotions & Deals | Discount stacking destroys margins (Pitfall 4) | Stacking rules before first promo type |
| Promotions & Deals | Timezone bugs in happy hour (Pitfall 11) | All times in IST, test midnight crossings |
| Marketing & Content | WhatsApp bot gets banned (Pitfall 8) | Separate number or skip WhatsApp for bulk |
| Marketing & Content | Generated images look bad (Pitfall 10) | Fixed templates, brand guidelines, preview |

---

## Sources

- [Beyond if (stock > 0): Handling Race Conditions in E-Commerce](https://dev.to/aaditya_efec6eedf3319cec3/beyond-if-stock-0-handling-race-conditions-in-high-traffic-e-commerce-systems-481) — concurrency patterns for stock decrement
- [How to Avoid the Top 7 Restaurant Inventory Mistakes](https://www.restaurant365.com/blog/how-to-avoid-the-top-7-restaurant-inventory-mistakes/) — unit confusion, cycle counting, ingredient tracking
- [Restaurant Inventory Management Best Practices 2025](https://supy.io/blog/restaurant-inventory-management-best-practices-a-complete-2025-guide-for-managers) — countable vs. bulk items
- [Issues with Promotional Pricing: Pitfalls](https://competera.ai/resources/articles/problems-of-promotional-pricing) — margin erosion, customer behavior
- [Promotion Stacking: Definition, Logic & Margin Control](https://www.voucherify.io/glossary/promotion-stacking) — stacking rules and guardrails
- [WhatsApp Business 2025 Update: New Restrictions](https://blog.zepic.com/article/whatsapp-business-april-2025-update-new-restrictions-whatsapp-broadcast-limit-explained) — broadcast limits, marketing template requirements
- [How to Send Bulk WhatsApp Messages Without Getting Banned](https://www.spurnow.com/en/blogs/how-to-send-bulk-whatsapp-messages) — rate limiting, opt-in requirements
- [Print Receipt From Web App to Thermal Printer](https://dev.to/streetcommunityprogrammer/street-programmer-print-receipt-from-online-pos-web-app-to-local-printer-1hnl) — ESC/POS web integration challenges
- [ESC/POS: Integrating Point of Sale Printers](https://brightinventions.pl/blog/esc-pos-integrating-point-of-sale-printers/) — printer model variance, documentation gaps
- [Coffee Shop Inventory Software Best Practices](https://pos.toasttab.com/blog/on-the-line/coffee-shop-inventory-software) — small cafe specific considerations
- [Discount Stacking and Calculation Rules](https://helpcenter.shoplazza.com/hc/en-us/articles/47060137498137-Discounts-Understanding-Discount-Stacking-and-Calculation-Rules) — stacking implementation patterns
