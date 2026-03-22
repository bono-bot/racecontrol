# Phase 158: Marketing & Content - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Auto-generate promo graphics for Instagram, WhatsApp broadcast of promo messages to customer list. Final phase of v19.0 cafe milestone.

</domain>

<decisions>
## Implementation Decisions

### Promo Graphics Generation
- Use satori + @resvg/resvg-js for JSX-to-PNG pipeline (no headless browser needed)
- Templates for: daily menu, active promo, new item announcement
- Racing Point brand identity: #E10600 red, #1A1A1A black, Montserrat font
- Admin clicks "Generate" on a promo → gets downloadable PNG for Instagram stories/posts
- Server-side generation via Node.js script (not Rust — satori is JS-only)

### WhatsApp Broadcast
- Use a SEPARATE WhatsApp number from the operational bot (research flagged ban risk)
- Or use existing number with rate limiting if separate number not available
- Admin triggers broadcast from admin panel → sends to all customers with phone numbers
- Message includes promo name, description, and optional image

### Admin UI
- "Marketing" tab on /cafe admin page
- Generate graphic button per promo
- Broadcast button with customer list preview and confirmation
- Download generated images

### Claude's Discretion
- Exact graphic template layouts
- Rate limiting for WhatsApp broadcasts
- Whether to store generated images or generate on-demand
- Broadcast message template formatting

</decisions>

<code_context>
## Existing Code Insights

### Reusable Assets
- `crates/racecontrol/src/cafe_promos.rs` — promo data for graphic content
- `crates/racecontrol/src/cafe.rs` — menu items for graphic content
- `crates/racecontrol/src/whatsapp_alerter.rs` — Evolution API pattern
- `web/src/app/cafe/page.tsx` — admin page with tabs

### Integration Points
- New Node.js script for image generation (satori + resvg)
- `cafe.rs` or new endpoint — trigger generation, trigger broadcast
- `page.tsx` — add Marketing tab

</code_context>

<specifics>
## Specific Ideas

No specific requirements.

</specifics>

<deferred>
## Deferred Ideas

None.

</deferred>
