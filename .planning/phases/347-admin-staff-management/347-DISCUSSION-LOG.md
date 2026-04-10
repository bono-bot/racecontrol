# Phase 347: Admin Staff Management - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md -- this log preserves the alternatives considered.

**Date:** 2026-04-10
**Phase:** 347-admin-staff-management
**Areas discussed:** PIN Change UX Flow, Endpoint Architecture, Error Recovery, Feature Flag Strategy
**Mode:** --auto (all areas auto-selected, recommended defaults chosen)

---

## PIN Change UX Flow

| Option | Description | Selected |
|--------|-------------|----------|
| Staged progress stepper | Inline stepper in modal showing each verification step with checkmarks | ✓ |
| Simple spinner | Single loading spinner with success/error at end | |
| Confirmation dialog | Two-step: confirm intent, then show result | |

**User's choice:** [auto] Staged progress stepper (recommended default)
**Notes:** Matches STAFF-08 requirement verbatim. Existing manage page has inline edit pattern; this replaces only the PIN edit flow.

---

## Endpoint Architecture

| Option | Description | Selected |
|--------|-------------|----------|
| New handler in routes.rs | Flat handler following existing module pattern, orchestration inline | ✓ |
| New staff_admin.rs module | Separate module for admin staff operations | |
| Extend existing update handler | Add safe-pin logic to existing PUT /staff/{id} | |

**User's choice:** [auto] New handler in routes.rs (recommended default)
**Notes:** Follows CONVENTIONS.md flat module organization. Two new endpoints: change-pin + sync/pull-now.

---

## Error Recovery

| Option | Description | Selected |
|--------|-------------|----------|
| Error banner with specific message | Show exactly which step failed + contact info | ✓ |
| Auto-retry with backoff | Automatically retry failed sync steps up to 3 times | |
| Toast-only notification | Simple error toast, user must retry manually | |

**User's choice:** [auto] Error banner with specific message (recommended default)
**Notes:** Matches STAFF-09 requirement. No auto-retry -- explicit error safer for staff PIN operations.

---

## Feature Flag Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| FEATURE_STAFF_PIN_UI env var + deploy gate | Flag defaults off, pre-deploy script checks Phase 343 shipped | ✓ |
| Runtime feature flag from DB | Feature flag table in racecontrol, toggled via admin settings | |
| No flag, hard dependency check only | Deploy gate script blocks but no runtime toggle | |

**User's choice:** [auto] FEATURE_STAFF_PIN_UI env var + deploy gate (recommended default)
**Notes:** Matches STAFF-10 + DEP-04. Existing manage page works without flag -- safe fallback.

---

## Claude's Discretion

- Rust module placement for change_staff_pin_safe
- HTTP client for venue->cloud forwarding
- Admin Next.js proxy route structure
- Optional last_pin_change_at column
- Loading skeleton reuse from Phase 354

## Deferred Ideas

None
