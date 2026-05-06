---
title: V2 Design Handoff — Layer 1: Foundation + POS .130 Staff Terminal
authored: 2026-05-06 ~16:30 IST
author: james (RacingPoint AI)
target_consumer: Claude Code design (frontend-design skill / ui-ux-pro-max skill / gsd-ui-researcher subagent)
layer: 1 of 7
purpose: Most-important + most-foundational design layer; establishes design system + tech stack + component primitives + API contracts that ALL subsequent layers (PWA / .23 portal / pod display / kiosk / chef display / admin) inherit. POS .130 is the vertical slice that exercises the foundation.
status: DRAFT for Captain review before ship to design
captain_authorization_needed: yes (1 verb to ship to design skill)
composes_with:
  - PACT-20260506-001 DRAFT (Phase 1 PACT-001 wire-up — implementation PACT for the surfaces designed here)
  - V2 customer workflows consolidated 2026-05-03 (5 base scenarios + 6 missed-resolved + 30-feature V2.0 list)
  - session_handoff PRIMARY 2026-05-06 (§AMEND-1 → §AMEND-4.H locks)
  - Phase 91 UI-SPEC convention (.planning/phases/91-session-experience/91-UI-SPEC.md — design-system inheritance source)
  - racecontrol/CLAUDE.md Brand Identity (LOCKED) + Standing Rules
---

# V2 Design Handoff — Layer 1: Foundation + POS .130 Staff Terminal

> **Read-order for design agent**: §1 (mission) → §2 (tech stack) → §3 (foundation — REUSED everywhere) → §5 (POS surface — concrete vertical) → §9 (anti-patterns) → §10 (acceptance). §4 (substrate) and §6 (reuse matrix) inform integration. §7 (subsequent layers) is for cross-layer planning, not this layer's scope. §8 has open decisions Captain must answer first.

---

## §1 — Layer overview + V2 mission frame

### Why this layer is FIRST

Per V2 customer workflows §2 Scenario 1, the **first operational customer-touch in V2.0 is staff at POS .130 looking up a customer by phone (CIRS lookup)**. CR-1 locks "V2.0 is staff-driven, not customer-self-serve" — every customer interaction goes through staff at POS .130 first. POS is therefore the operational hub: without it, no Scenario 1-5 can execute end-to-end.

POS .130 is **also** the surface that exercises the most foundational primitives:
- CIRS lookup → ProfilePreview composite (reused on .23 portal, admin panel, future PWA)
- Wallet top-up payment flow (reused for cafe ordering, mid-session top-up, refunds)
- Cafe order placement (reused for WhatsApp ordering, chef display)
- Receipt + tax-invoice rendering (reused for admin reporting, PWA history)
- Staff PIN auth pattern (reused on .23 portal + chef display + admin)
- Design tokens, component primitives, API conventions (reused EVERYWHERE)

**Layer 1 = Foundation (the design system, component primitives, API conventions, auth model) + POS .130 Staff Terminal (the vertical slice that proves and exercises the foundation).**

### V2 product frame (compressed for design context)

- **Single-roof venue**: indoor sim-racing + PS5 + cafe under one roof; staff-driven operation in V2.0
- **Customer journey**: staff greets → PWA registration (or POS register-on-behalf) → POS payment → wallet credit purchase → staff launches sim racing on .23 portal → customer plays at pod → balance runs out OR session ends → optional cafe → leave
- **Wallet Framing C (LOCKED)**: credits = Single-Purpose Voucher; 18% GST charged at top-up; redeemable ONLY for sim racing + PS5 (POS hard-blocks credit-for-cafe); never expires to customer
- **Billing Way A additive tier ladder (LOCKED §AMEND-3)**: cumulative-across-multi-experience-session, paused-between, 5min cliff (sub-locked per §AMEND-2). Vivek 12-step example: 30 × ₹25 + 30 × ₹20 + 90 × ₹15 = ₹2,700 / 150min
- **Failure handling first-class (CR-3)**: customer charged only for time used; apology credits issued for our-fault failures
- **Customer service > efficiency**: design choices favor clarity, error-prevention, audit trail, recovery paths
- **Audit-everywhere principle**: every CIRS lookup writes to cirs_lookup_audit; every wallet event writes to ledger; every pricing change writes to pricing_history. UI must NOT hide audit failures from staff (silent retry = bad).

### Anti-V1 framing (what V2 design must SOLVE that V1 didn't)

V1 venue is currently CLOSED (V1 failed). Per §S-61 V1 failure-mode investigation, recurring V1 antipatterns the design must address:
- Organ silos (POS / .23 / Kiosk acted independently — no shared session state)
- Manual fallbacks bypassing ratified flows (staff used cash + paper ledger when system failed)
- Features-on-shaky-foundation (UI changes deployed without backend contract validation)
- "Code complete" treated as "deployed" without venue UAT verification
- Pod display state diverged from .23 portal state (customer saw different reality than staff)

Design implication: V2 surfaces must REINFORCE shared state visibility (staff and customer see consistent reality), prevent bypass paths (no "edit this entry" backdoors), and surface system errors visibly (no silent fallbacks).

---

## §2 — Tech Stack (NON-NEGOTIABLE — locked for ALL layers)

| Concern | Lock | Source-of-truth |
|---|---|---|
| Framework | **Next.js 16.1.6** App Router | `web-v2/package.json` |
| Language | **TypeScript ^5** strict | `web-v2/tsconfig.json` |
| React | **19.2.3** | `web-v2/package.json` |
| Styling | **Tailwind CSS 4** with `@theme` tokens — NO shadcn/ui, NO component library | Phase 91 UI-SPEC convention; pwa/src/app/globals.css |
| Icons | **Inline SVG** (no Lucide, no Heroicons, no icon library) | Phase 91 UI-SPEC convention |
| Forms | React Hook Form + Zod (Q-DECISION-3 — see §8) | TBD by Captain |
| Server state | TanStack Query v5 (Q-DECISION-4 — see §8) | TBD by Captain |
| Client state | React hooks only (NO Zustand, NO Jotai, NO Redux for Phase 1) | Kaizen — substrate sufficient |
| Date/time | `date-fns` (LIGHT) — must format to IST (UTC+5:30) | Required by CLAUDE.md timezone rule |
| Charts | NONE in Layer 1 (admin reports = Layer 6) | Layer-scope discipline |
| HTTP transport | `fetch()` (App Router built-in) — wrapped in TanStack Query | Standard |
| Tests | Vitest + React Testing Library + Playwright (E2E) | Convention from existing PWA |
| Env vars | `NEXT_PUBLIC_*` baked at build (CLAUDE.md rule — grep ALL refs after change) | Required |
| basePath | `/v2` (web-v2 only; staff terminal mounts under this) | next.config.ts |
| Port | 3500 (web-v2 dev/prod) | Locked |
| Output | `standalone` (deploy substrate) | Locked |

### Anti-stack reminders

- **DO NOT** install `shadcn-ui`, `@radix-ui`, `@headlessui/react`, `@chakra-ui`, `mui`, `antd`, etc. Project convention is utility-first Tailwind + custom primitives.
- **DO NOT** install icon libraries (`lucide-react`, `react-icons`, `heroicons`, etc.). Use inline SVG with currentColor.
- **DO NOT** install animation libraries (`framer-motion`, `react-spring`). Use Tailwind transitions only — POS staff don't need motion.
- **DO NOT** install state libraries beyond TanStack Query (Q-DECISION-4). React hooks are sufficient.
- **DO NOT** import shared design tokens via relative path (V1 web Turbopack blocker per quarantine-discipline §2 of PACT-20260503-001). Use NPM workspace pattern (deferred Phase 0.2+) OR define tokens locally in `web-v2/src/app/globals.css` (Phase 1 lean default — duplicate the 6 rp-* tokens).

---

## §3 — Design System Foundation (REUSED across ALL layers)

### §3.1 Brand identity (LOCKED — racecontrol/CLAUDE.md)

| Token | Hex | Usage |
|---|---|---|
| `--color-rp-red` | `#E10600` | Racing Red — primary action, brand accent, active states |
| `--color-rp-red-light` | `#FF1A1A` | Hover state for primary action |
| `--color-rp-dark` | `#1A1A1A` | Asphalt Black — primary background (dark-mode-only) |
| `--color-rp-card` | `#222222` | Card / surface elevated 1 |
| `--color-rp-border` | `#333333` | Border, divider, scroll track |
| `--color-rp-grey` | `#5A5A5A` | Gunmetal Grey — secondary text, disabled state |

**Status colors** (extension — define in foundation, not invented per surface):

| Token | Hex | Usage |
|---|---|---|
| `--color-rp-success` | `#10B981` | Success state (payment recorded, CIRS lookup found) |
| `--color-rp-warning` | `#F59E0B` | Warning (Indian-mobile-prefix gate, ambiguous phone) |
| `--color-rp-danger` | `#DC2626` | Error (InvalidPhone, payment fail, audit fail) |
| `--color-rp-info` | `#3B82F6` | Info (system status, tooltip neutral) |

**DEPRECATED — DO NOT USE**: orange `#FF4400` (V1 brand orange) — explicitly removed per CLAUDE.md.

### §3.2 Typography

- **Body**: Montserrat (Google Fonts) + system-ui fallback stack
- **Headers**: Enthocentric (custom font — see §3.2.1 below) + Montserrat fallback
- **Mono**: `font-mono` Tailwind default (for lap times, timestamps, monetary amounts)
- **Weights**: 400 (regular) and 700 (bold) ONLY — NO 300/500/600/800/900 (Phase 91 convention)

#### §3.2.1 Enthocentric font sourcing

Captain decision needed (Q-DECISION-1 — see §8). Options:
- (a) Self-host woff2 in `web-v2/public/fonts/` if a license file exists at `racecontrol/assets/`
- (b) Substitute Montserrat-bold for headers as Layer 1 default (smallest reversible — fonts can be retrofitted)
- (c) Skip entirely, use Montserrat 700 everywhere

**Layer 1 lean default**: option (b) — use Montserrat 700 for headers; revisit when Enthocentric license confirmed. This is reversible at any time.

#### §3.2.2 Type scale (inherited from Phase 91 + extended for POS)

| Role | Size | Weight | Line height | Tailwind | Usage |
|---|---|---|---|---|---|
| Caption | 11px | 400 | 1.4 | `text-[11px]` | Tooltips, audit trail meta, rate-card secondary text |
| Label | 12px | 400 | 1.4 | `text-xs` | Field labels, badge text |
| Body | 14px | 400 | 1.5 | `text-sm` | Default content text, table cells |
| Body-bold | 14px | 700 | 1.5 | `text-sm font-bold` | Emphasis within body |
| Subheading | 16px | 700 | 1.4 | `text-base font-bold` | Card headers, section titles |
| Heading | 20px | 700 | 1.2 | `text-xl font-bold` | Page section headings |
| Page-title | 24px | 700 | 1.2 | `text-2xl font-bold` | Page H1 |
| Display | 36px | 700 | 1.0 | `text-4xl font-bold` | Hero amounts (wallet balance card-front, payment-success amount) |
| Money | varies | 700 + `font-mono` | — | — | All monetary amounts always use `font-mono` (audit + reconciliation visual cue per CLAUDE.md "money in paise" doctrine) |

### §3.3 Spacing scale (inherited from Phase 91 — exact match)

| Token | Value | Tailwind | Usage |
|---|---|---|---|
| xs | 4px | `space-1` / `gap-1` / `p-1` | Icon gaps, badge inner gap |
| sm | 8px | `space-2` / `gap-2` / `p-2` | Compact element spacing, label-to-value gap |
| md | 16px | `space-4` / `gap-4` / `p-4` | Default card internal padding, page horizontal padding |
| lg | 24px | `space-6` / `gap-6` / `p-6` | Section spacing |
| xl | 32px | `space-8` / `gap-8` / `p-8` | Major section breathing room |

**POS-specific exceptions** (touch-screen):

| Value | Tailwind | Justification |
|---|---|---|
| 48px touch-target min | `min-h-[48px]` `min-w-[48px]` | POS .130 is touchscreen; 44px (mobile) is too small for staff-with-gloves or fast operation |
| 64px primary action button | `h-16` | Confirm-payment, Start-session, Submit — explicit primacy |
| 72px numpad button | `h-[72px] w-[72px]` rounded-full | Inherited from PWA keypad-btn pattern (pwa/globals.css:51-69) |

### §3.4 Layout + viewport

| Surface (Layer 1) | Resolution | Orientation | Viewport meta |
|---|---|---|---|
| POS .130 staff terminal | **1920×1080** | landscape | `width=1920, initial-scale=1, user-scalable=no` |
| (Phase 1 single-resolution scope; touch-pad-with-keyboard fallback supported in HTML — Q-DECISION-2 see §8) | | | |

**NO responsive design in Layer 1 POS** — fixed 1920×1080 only. POS .130 hardware is fixed. Per kaizen-discipline + V2 customer workflows §1 surface table, POS .130 is the deterministic terminal.

Mobile / tablet / different resolutions are LATER LAYERS:
- Layer 2 PWA = 375px mobile primary (tablet 768px deferred V2.1)
- Layer 3 .23 portal = 1024px+ responsive
- Layer 4 pod display = 1280×720 fixed
- Layer 5 kiosk = 1280×1024 fixed
- Layer 6 chef display = 1920×1080 fixed
- Layer 7 admin panel = 1280px+ responsive

### §3.5 Iconography (inline SVG only)

Per Phase 91 UI-SPEC: NO icon library. Use inline SVG with `currentColor` for fill/stroke, sized via Tailwind `w-* h-*`.

Layer 1 icon inventory (author SVGs in `web-v2/src/components/icons/`):
- `<PhoneIcon />` (CIRS lookup, profile)
- `<WalletIcon />` (wallet balance, top-up)
- `<CashIcon />` (payment method)
- `<UpiIcon />` (payment method — UPI logo or "₹" symbol)
- `<CardIcon />` (payment method)
- `<ReceiptIcon />` (tax invoice)
- `<UserIcon />` (customer profile, walk-in guest)
- `<UserPlusIcon />` (register new customer)
- `<CheckIcon />` (success, confirm)
- `<XIcon />` (close, cancel, error)
- `<AlertIcon />` (warning, ambiguous)
- `<ClockIcon />` (last-visit, session-time)
- `<CafeIcon />` (cafe order — coffee cup)
- `<PauseIcon />` / `<PlayIcon />` (mid-session controls)
- `<LockIcon />` (staff PIN gate)
- `<LogoutIcon />` (end session)
- `<ChevronRightIcon />`, `<ChevronLeftIcon />`, `<ChevronDownIcon />` (navigation)
- `<SearchIcon />` (lookup, filter)
- `<FilterIcon />` (admin)
- `<RefreshIcon />` (pull-to-refresh, retry)
- `<RacingPointLogo />` (brand mark — at top-left of POS)

Standard icon size: `w-5 h-5` (20px) for inline; `w-6 h-6` (24px) for primary buttons; `w-12 h-12` (48px) for hero state (success badge).

### §3.6 Motion

- Tailwind transitions only: `transition-colors`, `transition-transform`, `transition-opacity`
- Default duration: 150ms (POS — fast staff feedback)
- Easing: default Tailwind `ease-out`
- **NO animations**: no `animate-spin`, no `animate-pulse`, no framer-motion. The only acceptable "spinner" on POS = simple text "Loading..." or a 3-dot CSS-only stable pulse for waits >500ms.

Per Phase 91 + CR-3 customer-service-priority + POS staff-in-motion ergonomics — animations distract, slow down operation, and create false-positive "still loading" perception.

### §3.7 Component primitive inventory (Foundation — design these THOROUGHLY in Layer 1)

These are reused across Layers 2-7. Authoring them in Layer 1 establishes the pattern.

| Primitive | Anchor pattern | Layer 1 instances | Reuse in later layers |
|---|---|---|---|
| `<Button>` | size: `sm` (32px) / `md` (40px) / `lg` (48px) / `xl` (64px); variant: `primary` (rp-red) / `secondary` (rp-card) / `ghost` (transparent) / `danger` (rp-danger) | All POS actions | All layers |
| `<Input>` | text/number/tel/email; with optional `<label>`, helper-text, error-text; rp-card bg, rp-border border, focus:ring-rp-red | Phone input, payment amount, customer name | All layers |
| `<NumPad>` | 12-button (3×4) grid; 72px circular buttons; clear + delete keys; emits string | Phone entry, payment amount, PIN entry | Kiosk + PWA register |
| `<Card>` | rp-card bg, rp-border border, p-4, rounded-md | All container surfaces | All layers |
| `<Modal>` | full-screen-overlay (POS); rp-dark backdrop 80% alpha; rp-card content | Confirmation dialogs, register-on-behalf form | All layers |
| `<Badge>` | inline pill with status color | "discount_ineligible", "walk-in", "active session", "pending payment" | All layers |
| `<Toast>` | top-right slide-in; auto-dismiss 4s; action-link option | Payment success, error, info | All layers |
| `<Spinner>` | text "Loading..." OR 3-dot CSS pulse | Async states | All layers |
| `<EmptyState>` | icon + title + helper text + CTA | "No customer found", "No active sessions", "No cafe orders" | All layers |
| `<ErrorBoundary>` | wraps every route segment; logs to console + audit; shows recovery CTA | App-wide | All layers |
| `<Money>` | renders i64 paise as ₹X.XX with font-mono; right-aligned in tables | Wallet balance, payment amounts, tier rates | All layers |
| `<Phone>` | renders E.164 as "+91 98765 43210" formatted | ProfilePreview, audit trail | All layers |
| `<DateTime>` | renders UTC ISO to IST formatted ("06 May 2026 16:13 IST") | Last-visit, audit-row ts | All layers |
| `<StaffPinGate>` | full-screen PIN entry; calls `POST /api/v1/auth/staff/pin`; sets session cookie | Wraps every POS route | Reused on .23 portal, chef display |

**Composite primitives** (Layer 1 first instances; reused later):

| Composite | Layer 1 use | Reused later |
|---|---|---|
| `<ProfilePreviewCard>` | After CIRS lookup found | .23 portal pre-launch confirmation; admin panel customer search; PWA self-view |
| `<WalletBalanceCard>` | POS top-up flow + sidebar state | PWA wallet view; admin customer detail |
| `<WalkInGuestDropdown>` | POS lookup fallback (Path 2) | .23 portal walk-in fallback |
| `<PaymentMethodSelector>` | Top-up + cafe-order | Layer 2 PWA top-up (V2.1); admin refund flow |
| `<ReceiptCard>` | Post-payment confirmation | PWA history; WhatsApp e-receipt; admin reprint |
| `<CafeOrderRow>` | Cafe ordering page | Chef display order queue; WhatsApp order confirmation |
| `<TierLadderTable>` | Pre-payment estimate ("you'll get N min for ₹X"); admin pricing editor | All payment surfaces |
| `<SessionRow>` | Active sessions list at POS | .23 portal active-sessions view; admin reporting |

### §3.8 Accessibility baseline (WCAG AA)

- Contrast: text-primary on rp-dark = 18.2:1 ✓; text-rp-grey (#5A5A5A) on rp-dark (#1A1A1A) = 4.6:1 ✓; rp-red (#E10600) on rp-dark = 4.7:1 ✓
- Keyboard navigation: all interactive elements tab-reachable; focus ring visible (`focus:ring-2 focus:ring-rp-red focus:ring-offset-2 focus:ring-offset-rp-dark`)
- Screen reader: aria-label on icon-only buttons; aria-live on toast region; aria-invalid on error inputs
- Touch targets: ≥48px (POS spec)
- Focus management: modal traps focus; closing modal returns focus to trigger
- Reduced motion: respect `prefers-reduced-motion` for the 150ms transitions (none → instant)

POS-specific note: staff are NOT visually impaired by job requirement; aria-live is courtesy not necessity. Emphasis is on touch-target size and error-state contrast (4.5:1 minimum on error text).

### §3.9 globals.css scaffolding (Layer 1 lean default — duplicate-not-import)

The Layer 1 `web-v2/src/app/globals.css` should look like this (SUBSET of pwa/src/app/globals.css, with Layer 1 extensions):

```css
@import "tailwindcss";

@theme {
  /* Brand (locked — racecontrol/CLAUDE.md) */
  --color-rp-red: #E10600;
  --color-rp-red-light: #FF1A1A;
  --color-rp-dark: #1A1A1A;
  --color-rp-card: #222222;
  --color-rp-border: #333333;
  --color-rp-grey: #5A5A5A;

  /* Status (Layer 1 extension) */
  --color-rp-success: #10B981;
  --color-rp-warning: #F59E0B;
  --color-rp-danger: #DC2626;
  --color-rp-info: #3B82F6;
}

@layer base {
  * { -webkit-tap-highlight-color: transparent; }
  html { color-scheme: dark; }
  body {
    background-color: #1A1A1A;
    color: #FFFFFF;
    font-family: 'Montserrat', system-ui, -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif;
    -webkit-font-smoothing: antialiased;
    overscroll-behavior-y: contain;
  }

  ::-webkit-scrollbar { width: 4px; }
  ::-webkit-scrollbar-track { background: transparent; }
  ::-webkit-scrollbar-thumb { background: #5A5A5A; border-radius: 2px; }
}

@layer components {
  .keypad-btn {
    display: flex; align-items: center; justify-content: center;
    width: 72px; height: 72px;
    border-radius: 50%;
    background: #333333;
    color: #FFFFFF;
    font-size: 24px; font-weight: 700;
    transition: background 0.15s;
    user-select: none; cursor: pointer;
  }
  .keypad-btn:active { background: #E10600; }
  .keypad-btn[disabled] { opacity: 0.4; cursor: not-allowed; }

  /* POS focus ring */
  .focus-ring {
    @apply focus:outline-none focus:ring-2 focus:ring-rp-red focus:ring-offset-2 focus:ring-offset-rp-dark;
  }
}
```

This is the EXACT starting state for Layer 1. Do NOT re-derive tokens — use these.

---

## §4 — Phase 0 Substrate (already shipped — design INTEGRATES with this; do NOT re-design)

These Rust types are MERGED on origin/main and define the API contract that Layer 1 MUST conform to. Frontend MUST NOT request schema changes — substrate is canonical.

### §4.1 CIRS API (`crates/v2-db/src/cirs.rs` — committed in PR #61 squash `483562ac`)

```rust
// LookupInput — exactly one variant per call
pub enum LookupInput {
    Phone { phone: String },          // ACTIVE in v2.0
    QrPayload { payload: String },    // PLUMBED-DISABLED v2.0 (M3 feature flag)
    NfcTagId { tag_id: String },      // PLUMBED-DISABLED v2.0 (M4 feature flag)
    WalkInGuestId { guest_id: u8 },   // ACTIVE — guest_id ∈ {1, 2}
}

// LookupResult — Phase 0 returns audit tag only; Phase 1 (this layer) populates ProfilePreview
pub enum LookupResult {
    Found { customer_id: String },
    NotFound,
    Error { message: String },
}

// canonicalize_phone — relaxed-input, strict-on-substrate-write
// 4 rules per §3.1:
//   "+91XXXXXXXXXX" (E.164)              → already canonical
//   10-digit-all-digit                    → "+91" + input (auto-prefix Indian)
//   11-digit-starts-"0"                   → REJECT AmbiguousPhone (stale STD prefix)
//   11+-digit-no-"+"                      → REJECT AmbiguousPhone (missing country code)
//   anything-else                         → REJECT InvalidPhone
pub fn canonicalize_phone(input: &str) -> Result<String, CirsError>;

// Errors UI must surface:
pub enum CirsError {
    InvalidPhone(String),
    AmbiguousPhone(String),
    Sqlx(sqlx::Error),
}
```

### §4.2 ProfilePreview shape (Phase 1 wire-up populates this — Layer 1 designs it)

Per PACT-20260505-001 §3:

```typescript
interface ProfilePreview {
  customer_id: string;            // UUID
  primary_phone: string;          // E.164 "+91XXXXXXXXXX"
  name: string | null;            // may be null for register-pending
  profiles: Array<{               // family multi-profile (1 primary + up to 3 sub)
    profile_id: string;
    name: string;
    is_default: boolean;
    discount_ineligible: boolean;
  }>;
  wallet_balance_credits: number; // i64 — 1 credit = ₹1 face value
  last_visit_ts: string | null;   // ISO8601 UTC; UI converts to IST
  arrival_history_count_30d: number;
  discount_ineligible: boolean;   // top-level (true for walk-in guests + flagged)
}
```

### §4.3 Wallet API (`crates/v2-db/src/wallets.rs`)

Money is stored in PAISE (i64) end-to-end. Credits are i64 count where 1 credit = ₹1 face value. NO floats anywhere.

```rust
pub struct Wallet {
    pub id: Uuid,
    pub customer_id: Uuid,
    pub balance_credits: i64,
    pub last_activity_at: DateTime<Utc>,
    pub breakage_recognized_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct WalletTopup {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub credits_purchased: i64,
    pub gst_collected_paise: i64,
    pub amount_paid_paise: i64,
    pub gst_rate_bps: i32,           // 1800 = 18%
    pub payment_method: PaymentMethod, // Cash | Upi | Card
    pub payment_ref: Option<String>,   // UPI txn id, last-4 card digits
    pub staff_id: String,              // FK to staff(id) — TEXT not UUID
    pub pos_id: String,                // "POS-130" canonical
    pub tax_invoice_no: String,        // GST-compliant invoice number
    pub created_at: DateTime<Utc>,
}
```

UI implication: `<Money>` primitive renders paise as ₹X.XX; payment-method selector emits the enum; tax_invoice_no is generated server-side and surfaced on receipt.

### §4.4 Staff PIN auth substrate (PACT-018 Phase 0.5c — MERGED `3119da30`)

```sql
-- staff table (TEXT primary key, NOT UUID)
CREATE TABLE staff (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  phone TEXT NOT NULL,
  pin TEXT NOT NULL,                                  -- raw V1 contract per security-debt-ledger
  role TEXT CHECK (role IN ('cashier', 'manager', 'admin', 'maintenance', 'inactive')),
  active INTEGER NOT NULL DEFAULT 1,
  last_login_at TEXT,
  -- ... other fields
);

CREATE INDEX idx_staff_active ON staff(active);
CREATE UNIQUE INDEX uniq_staff_phone_active ON staff(phone) WHERE active = 1;
CREATE UNIQUE INDEX uniq_staff_pin_active ON staff(pin) WHERE active = 1;
```

**Security debt acknowledged**: PIN is stored raw in V1 contract. Closure_phase = Phase-0.5c-AUTH (sibling sub-PACT for bcrypt-hardening). Layer 1 design assumes raw PIN substrate; UI must NEVER display PIN, log PIN, or include PIN in error messages. PIN entry on POS uses `<NumPad>` with masked input.

### §4.5 Audit substrate (cirs_lookup_audit — MERGED `483562ac`)

```sql
CREATE TABLE cirs_lookup_audit (
  id INTEGER PRIMARY KEY,
  ts TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
  staff_id TEXT NOT NULL REFERENCES staff(id),
  customer_id TEXT REFERENCES customers(id),         -- nullable for not_found / walk-in
  input_method TEXT CHECK (input_method IN ('phone','qr_payload','nfc_tag_id','walk_in_guest_id')),
  input_hash TEXT,                                   -- SHA256 of raw input; NULL for walk_in_guest
  result TEXT CHECK (result IN ('found','not_found','error'))
);
```

Layer 1 invariant: **every CIRS lookup writes ONE audit row regardless of outcome**. Frontend MUST NOT depend on this — it's substrate-side. But UI must surface audit failures (if the audit-write fails, that's a server error and lookup must fail closed, not silently succeed).

---

## §5 — Layer 1 Surface: POS .130 Staff Terminal

### §5.1 User context

- **User**: Staff cashier at reception (1-3 staff per shift)
- **Hardware**: 1920×1080 fixed touchscreen + keyboard fallback; Ethernet to LAN; positioned at counter
- **Location**: 192.168.31.130 (POS PC; per CLAUDE.md Network Map)
- **Tenancy**: single staff session at a time (PIN auth on entry; session expires on idle 30min OR explicit logout)
- **Frequency**: ~50-200 customer interactions per day at peak
- **Speed-priority**: every flow must be optimizable to <30s per customer for routine top-up

### §5.2 Information architecture (route map)

All routes mounted at `/v2/pos/*` (basePath `/v2` from next.config.ts).

```
/v2/pos/login                  — staff PIN gate (entry; renders if no session cookie)
/v2/pos                        — main dashboard (active sessions list + quick-actions)
/v2/pos/lookup                 — CIRS phone-lookup + ProfilePreview + Walk-In fallback
/v2/pos/register               — register on customer's behalf (Path B; called from /lookup NotFound)
/v2/pos/topup/[customer_id]    — wallet top-up flow (cash/UPI/card payment recording)
/v2/pos/cafe                   — cafe order placement (separate from credits per CR-5)
/v2/pos/cafe/[order_id]        — order detail / mark delivered
/v2/pos/billing                — manual PS5 billing + pending-payment tracker (CR-3 + missed scenario B)
/v2/pos/session/[session_id]   — active session detail (mid-session top-up, end session, view history)
/v2/pos/logout                 — explicit logout (clear session cookie)
```

Out of scope for Layer 1 (these are Layer 3+ on .23 portal): game launch, preset selection, MP lobby creation, pause/resume.

### §5.3 Per-screen scope

#### Screen 5.3.1 — `/v2/pos/login`

**Purpose**: Staff authenticates with PIN before any POS action.

**Layout**: full-screen `<Card>` centered (max-w-[480px]); RacingPointLogo at top; `<NumPad>` for PIN entry; submit button; error banner inline.

**State machine**:
```
IDLE → ENTER_PIN → SUBMITTING → (AUTHED → /v2/pos | ERROR → IDLE with error)
```

**Components**:
- `<RacingPointLogo />` (top, 64px)
- Heading "Staff Login" (text-2xl font-bold)
- `<Input>` PIN display (masked dots, read-only — entered via NumPad)
- `<NumPad />` (12-button)
- `<Button variant="primary" size="xl">Sign In</Button>`
- `<Toast />` for error ("Invalid PIN" / "Account inactive" / "Network error")

**Acceptance**:
- [ ] PIN entry via NumPad; backspace and clear keys present
- [ ] PIN masked as ●●●● regardless of length
- [ ] Submit calls `POST /api/v1/auth/staff/pin {pin}` → 200 sets HttpOnly session cookie; 401 renders error
- [ ] On 401: error toast, NumPad clears, focus returns to NumPad
- [ ] On 200: redirect to `/v2/pos`
- [ ] No PIN ever appears in DOM, console, network log (verify via DevTools)
- [ ] Tab-order: NumPad → Submit → (no skip)
- [ ] **NOT TESTED** in design: rate-limit / lockout policy (Captain Q-DECISION-5 — see §8)

#### Screen 5.3.2 — `/v2/pos` (dashboard)

**Purpose**: Operational hub. Staff sees venue state at a glance + initiates customer interactions.

**Layout** (1920×1080 split):
```
┌─────────────────────────────────────────────────────────────────────┐
│ [Logo] Racing Point V2 — POS .130          [Staff: name] [Logout] ▼│ ← header (h-16)
├──────────────────────────────────┬──────────────────────────────────┤
│                                  │                                  │
│  Active Sessions (left 60%)      │  Quick Actions (right 40%)       │
│                                  │                                  │
│  - <SessionRow> Pod 1 / J. Smith │  [Lookup Customer]    (xl button)│
│    ₹240 / 12:34 / Pause | End    │  [Register New]       (lg)       │
│  - <SessionRow> Pod 3 / A. Patel │  [Wallet Top-up]      (lg)       │
│    ₹180 / 08:12 / Pause | End    │  [Cafe Order]         (lg)       │
│  ...                             │  [PS5 Billing]        (md)       │
│  (scrollable)                    │  [Pending Payments]   (md)       │
│                                  │                                  │
│                                  │  ──────────────────              │
│                                  │  System Status                   │
│                                  │  • Server .23: ✓ healthy         │
│                                  │  • Pods online: 7/8              │
│                                  │  • Cloud sync: ✓                 │
│                                  │  • Last lookup: 2 min ago        │
└──────────────────────────────────┴──────────────────────────────────┘
```

**Components**:
- Header: `<RacingPointLogo />`, page title, `<Badge>` staff name + role, logout link
- Active Sessions list: `<SessionRow>` components (pod number, customer name, current bill, duration, pause/end actions)
- Quick Actions stack: large primary action ("Lookup Customer" — xl button), secondary actions (lg), tertiary (md)
- System Status: live polling (TanStack Query refetchInterval 10s); `<Spinner>` during refetch is implicit (data shows stale + new in-place)

**Data flows**:
- Active sessions: `GET /api/v1/sessions/active` (returns sessions array; UI subscribes via TanStack Query 5s polling)
- System status: `GET /api/v1/fleet/health` + cloud-health composite

**Acceptance**:
- [ ] Active sessions render within 500ms of route entry
- [ ] Polling refresh occurs every 5s without UI flash (TanStack Query `keepPreviousData`)
- [ ] Quick Actions are at least 48px tall touch targets
- [ ] System Status surfaces ANY anomaly visibly (red dot + name) — staff can see at a glance
- [ ] On network failure: stale data shown with "Last updated: Xs ago" banner; no auto-redirect to error page
- [ ] **NOT TESTED**: behavior when active-sessions count >20 (virtualization deferred Layer 6 admin)

#### Screen 5.3.3 — `/v2/pos/lookup`

**Purpose**: CIRS phone-lookup; the FIRST customer-touch surface. THIS is the screen Phase 1 PACT-001 wire-up centers on.

**Layout** (split view):
```
┌─────────────────────────────────────────────────────────────────────┐
│ [Logo] V2 POS .130 — Customer Lookup        [< Back] [Staff: name] │
├──────────────────────────────────┬──────────────────────────────────┤
│                                  │                                  │
│  Phone Lookup (left 50%)         │  Walk-In Fallback (right 50%)    │
│                                  │                                  │
│  ┌────────────────────────────┐  │  No phone? Use walk-in guest:    │
│  │ +91 _________ (input)      │  │                                  │
│  │ [NumPad]                   │  │  [<WalkInGuestDropdown>]         │
│  │                            │  │  • Walk-In Guest 1               │
│  │ [Lookup] (primary xl)      │  │  • Walk-In Guest 2               │
│  └────────────────────────────┘  │                                  │
│                                  │  Note: Walk-in guests are        │
│  Result area:                    │  flagged discount_ineligible.    │
│  → <ProfilePreviewCard /> on     │                                  │
│    Found                         │                                  │
│  → <NotFoundCTA /> on NotFound   │                                  │
│  → <LookupErrorBanner /> on Err  │                                  │
└──────────────────────────────────┴──────────────────────────────────┘
```

**State machine**:
```
IDLE → ENTERING → CANONICALIZING → LOOKING_UP →
   ├─ FOUND       → render <ProfilePreviewCard /> + [Bind Identity] CTA
   ├─ NOT_FOUND   → render <NotFoundCTA /> with [Register On Behalf] CTA
   ├─ INVALID     → render <LookupErrorBanner /> "Invalid phone format"; clear input ready
   ├─ AMBIGUOUS   → render <LookupErrorBanner /> warning "Looks like an STD-prefix; please confirm" + [Confirm and Lookup Anyway] override
                                                                                                                          ↓
   └─ NETWORK_ERR → render <LookupErrorBanner /> "Server unreachable"; [Retry]                                            ↓
                                                                                                  → emits LookupInput.Phone
              (Walk-In path)                                                                                               ↓
   IDLE → SELECT_GUEST → render <ProfilePreviewCard /> short-circuit (discount_ineligible: true)
```

**Components**:
- `<Input type="tel" inputMode="numeric" />` for phone — controlled, with 4-rule canonicalize on blur
- `<NumPad />` (12-button)
- `<Button variant="primary" size="xl">Lookup</Button>` — disabled until input non-empty
- `<ProfilePreviewCard />` — Found state (composite — see §3.7)
- `<NotFoundCTA />` — heading "No customer found"; subhead "Register on customer's behalf?"; `<Button>Register New →</Button>` routes to `/v2/pos/register?phone=<canonical>`
- `<LookupErrorBanner />` — uses status colors (warning for AmbiguousPhone, danger for InvalidPhone/network)
- `<WalkInGuestDropdown />` — labeled "Walk-In Guest 1" / "Walk-In Guest 2"; on select renders `<ProfilePreviewCard />` for the guest account (which already exists in substrate as a fallback record)

**NF-james-B Phase 1 UI gate (per PACT-20260506-001 §3)**:
- input.length === 10 && digit[0] ∈ {6,7,8,9} → AGREE (likely Indian mobile) — no warning
- input.length === 10 && digit[0] ∈ {0,1,2,3,4,5} → render `<LookupErrorBanner variant="warning" />` "This doesn't look like an Indian mobile — confirm or correct?" + override-and-lookup-anyway button
- WARN-only (NOT BLOCK) — staff override allowed for international iRacing customers

**Data flows**:
- Lookup: `POST /api/v1/cirs/lookup { method: "phone", value: "+91XXXXXXXXXX" }` → returns `ProfilePreview` (Found) | 404 NotFound | 400 InvalidPhone/AmbiguousPhone | 500 Error
- Audit: every lookup writes to `cirs_lookup_audit` server-side (transparent to UI)

**Acceptance**:
- [ ] Phone entry via NumPad OR keyboard (both supported)
- [ ] Canonicalize debounce: 300ms after last keystroke
- [ ] Lookup CTA disabled while input is empty
- [ ] Indian-mobile-prefix WARN renders inline below input on first non-{6,7,8,9} 10-digit input
- [ ] Found state renders `<ProfilePreviewCard />` within 500ms p95 (DoD §1.2)
- [ ] NotFound state: CTA routes to `/v2/pos/register?phone=...` preserving canonicalized phone
- [ ] InvalidPhone state: error red banner; input retained for correction
- [ ] AmbiguousPhone state: warning amber banner with override option
- [ ] Network error: red banner with retry; previous result NOT cleared (so staff can see what they had)
- [ ] Walk-In Guest 1/2 selection routes to short-circuit ProfilePreview (no API call needed for guest lookup — guest accounts are static)
- [ ] Bind Identity CTA on ProfilePreview: emits a session-binding event (links to active session if any) — Phase 2 wires the session-binding side; Layer 1 just routes to `/v2/pos/session/...` or back to dashboard
- [ ] **NOT TESTED**: staff PIN re-entry per lookup (Q-DECISION-6 — see §8); cache layer at POS (Q-DECISION-7); QR/NFC inputs (M3/M4 plumbed-disabled in v2.0)
- [ ] **NOT TESTED**: behavior when 2 staff are using POS simultaneously (only 1 POS hardware = single tenancy in V2.0)

#### Screen 5.3.4 — `/v2/pos/register`

**Purpose**: Path B — staff registers customer on their behalf when CIRS NotFound.

**Layout**: `<Card>` centered max-w-[640px]; form with required fields + optional fields.

**Required fields**: phone (pre-filled from query param `?phone=`), name. **Optional fields**: email, date of birth, marketing consent checkbox.

**Components**: `<Input>` for each field; `<Button variant="primary" size="xl">Register & Continue</Button>` → routes to `/v2/pos/topup/[customer_id]?just_registered=true` on success.

**Data flows**:
- Submit: `POST /api/v1/customers { phone, name, email?, dob?, marketing_consent }` → returns `customer_id`
- Idempotency: server-side check (re-using same phone returns existing customer_id with 200, not 409)

**Acceptance**:
- [ ] Phone is read-only (canonical format from previous step)
- [ ] Name required; min 2 chars
- [ ] Email optional but validated when provided
- [ ] DOB optional; date picker not required (free-text input acceptable for V2.0)
- [ ] Submit success routes to top-up flow with `?just_registered=true` banner
- [ ] Submit failure (network) shows error toast; form retained
- [ ] **NOT TESTED**: Aadhaar / GST identifier capture (Q-DECISION-8 — V2.1 scope)

#### Screen 5.3.5 — `/v2/pos/topup/[customer_id]`

**Purpose**: Wallet top-up — staff records cash/UPI/card payment, system issues credits.

**Layout** (3-column 1920×1080):
```
┌─────────────────────────────────────────────────────────────────────┐
│ [Logo] V2 POS .130 — Wallet Top-Up           [< Back] [Staff: name]│
├──────────────┬─────────────────────────────────┬─────────────────────┤
│              │                                 │                     │
│ Customer     │  Amount Entry (center)          │  Tier Estimate      │
│              │                                 │  (right)            │
│ <ProfilePrev │  ┌───────────────────────────┐  │                     │
│ iewCard      │  │  ₹  __________ (input)    │  │  <TierLadderTable  │
│ compact />   │  │                           │  │   selected={amount}│
│              │  │  [<NumPad>]               │  │   />               │
│ Current bal: │  │                           │  │                     │
│ <Money 480/> │  │  Payment Method:          │  │  At ₹X you get:    │
│              │  │  [<PaymentMethodSelector  │  │  • Y minutes        │
│              │  │   value=cash|upi|card />] │  │  • Z credits        │
│              │  │                           │  │  • +N bonus         │
│              │  │  Optional payment_ref:    │  │                     │
│              │  │  [<Input>]                │  │  GST 18% included   │
│              │  └───────────────────────────┘  │                     │
│              │                                 │                     │
│              │  [Confirm Payment] (xl danger ?)│                     │
└──────────────┴─────────────────────────────────┴─────────────────────┘
```

**State machine**: IDLE → ENTERING → SELECTING_METHOD → CONFIRMING → SUBMITTING → (SUCCESS → receipt | ERROR → toast)

**Components**:
- Left: `<ProfilePreviewCard variant="compact" />` (always visible during top-up — staff sees who they're crediting)
- Center: `<Input type="number" />` for amount (paise); `<NumPad />`; `<PaymentMethodSelector />`; optional `<Input>` for payment_ref (UPI txn id, last-4 card)
- Right: `<TierLadderTable />` showing what the customer gets at the entered amount (live update on input change)
- Confirmation: `<Button variant="primary" size="xl">Confirm Payment ₹X.XX</Button>`

**Data flows**:
- Tier estimate: `POST /api/v1/billing/estimate { credits: amount_in_credits }` → returns `{ tier_breakdown, gst_paise, total_paise }` (Way A additive)
- Submit: `POST /api/v1/wallet/topup { customer_id, amount_paid_paise, payment_method, payment_ref?, gst_rate_bps: 1800, staff_id, pos_id: "POS-130" }` → returns `WalletTopup` with `tax_invoice_no`
- On success: route to `/v2/pos/receipt/[topup_id]` (renders `<ReceiptCard />` printable)

**Acceptance**:
- [ ] Amount input accepts whole rupees (paise = ₹X * 100; UI displays ₹X)
- [ ] Tier-ladder updates within 200ms of input change
- [ ] Tax invoice number generated server-side (UI does NOT compute it)
- [ ] Confirmation button label includes the exact amount ("Confirm Payment ₹500.00")
- [ ] After confirm, success state shows receipt + auto-print prompt (browser print dialog)
- [ ] On payment failure: error toast; form retained; staff can adjust + retry
- [ ] Audit invariant: every successful top-up writes to `wallet_topups` table (server-side; UI does not need to verify)
- [ ] **NOT TESTED**: discount application (Q-DECISION-9 happy-hour / iRacing 20%); pricing snapshot mid-flight per §AMEND-3.II foundation/strategy/config layer (live mode default)

#### Screen 5.3.6 — `/v2/pos/cafe`

**Purpose**: Place cafe order — IMMEDIATE cash/UPI/card transaction, NEVER credit-redeemed (CR-5 hard rule).

**Layout** (split):
```
┌─────────────────────────────────────────────────────────────────────┐
│ [Logo] V2 POS .130 — Cafe Order              [< Back] [Staff: name]│
├──────────────────────────────────┬──────────────────────────────────┤
│ Menu Grid (left 65%)             │  Order Summary (right 35%)       │
│                                  │                                  │
│ [Coffee]  [Tea]  [Sandwich]      │  Customer (optional):            │
│ [Pasta]   [Snack][Cold Drink]    │  [<PhoneLookupInput compact />]  │
│ ...                              │  → if found, attaches order      │
│                                  │  → if not provided, anonymous    │
│ (categories + items)             │                                  │
│                                  │  Items:                          │
│                                  │  - 2× Coffee     ₹100            │
│                                  │  - 1× Sandwich   ₹150            │
│                                  │                                  │
│                                  │  Subtotal: ₹250                  │
│                                  │  GST: ₹45 (18%)                  │
│                                  │  Total: ₹295                     │
│                                  │                                  │
│                                  │  Delivery: ☐ Cafe seating  ☑ Hall│
│                                  │  (Q-S6 hybrid Opt C)             │
│                                  │                                  │
│                                  │  [<PaymentMethodSelector />]     │
│                                  │  [Confirm Order ₹295] (xl)       │
└──────────────────────────────────┴──────────────────────────────────┘
```

**Components**:
- Left: `<CafeMenuGrid />` (categories + items; tap to add); `<CafeOrderRow />` for each line
- Right: optional `<PhoneLookupInput variant="compact" />` (if customer wants order attached to profile per missed-scenario E retroactive-mapping); `<OrderSummary>`; delivery toggle (cafe-seating vs gaming-hall); `<PaymentMethodSelector>`; `<Button>Confirm</Button>`

**HARD CONSTRAINT (CR-5)**: No "use credits" option ANYWHERE on this screen. Cafe is decoupled from wallet entirely.

**Data flows**:
- Menu: `GET /api/v1/cafe/menu` → categories + items (cached in TanStack Query 5min)
- Submit: `POST /api/v1/cafe/orders { items, customer_id?, delivery_location: "cafe_seating"|"gaming_hall", payment_method, payment_ref? }` → returns `cafe_order` with order_number; chef display surfaces it
- Anonymous order: customer_id omitted; staff records phone optional for retroactive-map per scenario E

**Acceptance**:
- [ ] No "Use Credits" / "Pay from Wallet" button anywhere on cafe surface (CR-5 enforcement)
- [ ] Delivery toggle: gaming_hall (default; per Q-S6 Opt C delivery via staff-courier) vs cafe_seating
- [ ] Anonymous orders allowed — no phone required
- [ ] Phone lookup is optional; if provided + Found, order attaches to customer for retroactive-mapping
- [ ] Order submission triggers chef display update (Layer 5; Layer 1 doesn't render that)
- [ ] **NOT TESTED**: WhatsApp confirmation (Layer 5 chef + WhatsApp channel); print of Bill of Order (Print Module sub-PACT pending Q-PRINT-1/2/3)

#### Screen 5.3.7 — `/v2/pos/billing` (PS5 manual + pending payment tracker)

**Purpose**: Manual PS5 billing (CR-8 — software integration deferred V2.1) + pending-payment tracker (missed scenario B mid-session top-up + apology-credit ledger).

**Layout** (tabs):
- Tab 1: "PS5 Sessions" — list PS5 sessions with manual time entry; staff records start/end time + customer; system computes amount via Way A additive ladder
- Tab 2: "Pending Payments" — list customers with deferred-payment credits owed; staff marks paid when reconciled
- Tab 3: "Apology Credits" — list issued apology credits with reasoning + amount

**Components**:
- `<SessionRow variant="ps5">` — pod-equivalent row with manual time entry
- `<PendingPaymentRow>` — customer name + amount owed + days outstanding + [Mark Paid] CTA
- `<ApologyCreditRow>` — customer name + amount + reason category + ts + [Reissue] (read-only audit row)

**Data flows**:
- PS5 sessions: `GET /api/v1/ps5/sessions/active`; submit on end: `POST /api/v1/ps5/sessions/end { session_id, end_ts }`
- Pending: `GET /api/v1/wallet/pending-payments`; mark: `POST /api/v1/wallet/pending-payments/[id]/reconcile { payment_method, payment_ref? }`
- Apology: `GET /api/v1/wallet/apology-credits`

**Acceptance**:
- [ ] PS5 session start/end manually entered (no auto-detect — V2.1 scope)
- [ ] Pending payment list sorted by oldest-first; visual cue for >7 days (warning amber); >30 days (danger red)
- [ ] Mark paid is irreversible (confirm dialog: "Customer X has paid ₹Y? This cannot be undone")
- [ ] Apology credits list is read-only (issuing happens during mid-session via `/v2/pos/session/[id]`)

#### Screen 5.3.8 — `/v2/pos/session/[session_id]`

**Purpose**: Active session detail — mid-session top-up, apology credit issuance, end session, view customer history.

**Layout**: 3-section card stack
- Top: `<ProfilePreviewCard variant="compact" />` + active session metadata (pod, game, duration, current bill, wallet balance)
- Mid: actions stack — [Mid-session Top-up] (routes to /v2/pos/topup/[customer_id]?session=[id]); [Pause Session] (Layer 3 .23 portal owns; Layer 1 just shows status); [Issue Apology Credit] (modal — reason category + amount + confirm); [End Session] (confirm + final bill)
- Bottom: Recent sessions for this customer (last 5)

**Acceptance**:
- [ ] Mid-session top-up preserves session id in query param so post-top-up returns to session view
- [ ] Apology credit issuance modal: reason categories (Game crash / Hardware fault / Network outage / Other); amount input (paise); confirm + audit row written
- [ ] End session: confirms total bill (Way A additive); writes `session_ended_at`; routes to dashboard

### §5.4 Cross-screen patterns

#### Header / chrome (every POS route)

```
┌─────────────────────────────────────────────────────────────────────┐
│ [Logo h-10] Racing Point V2 — POS .130    [Staff: name (role)] [▼] │
└─────────────────────────────────────────────────────────────────────┘
```

- Logo: link to `/v2/pos`
- Center: page-title (text-xl font-bold)
- Right: staff badge + dropdown (Settings / Logout)
- Height: 64px fixed; sticky at top

#### Footer (status bar, every POS route)

```
┌─────────────────────────────────────────────────────────────────────┐
│ Server: ✓  Pods: 7/8  Cloud: ✓  Time: 16:13 IST  | v2.0.NN-build_id │
└─────────────────────────────────────────────────────────────────────┘
```

- Live system status (polled 10s)
- IST clock (UTC + 5:30; per CLAUDE.md timezone rule)
- Version + build_id (read from `/api/v1/health` on mount; helps debugging)

#### Error boundaries (every route segment)

`<ErrorBoundary>` wraps every page. On uncaught exception:
1. Log to console
2. Send to `/api/v1/log/frontend-error` (best-effort)
3. Render `<EmptyState>` "Something went wrong" + [Retry] (page refresh) + [Back to Dashboard]
4. Preserve URL so retry works

### §5.5 State management (Layer 1 conventions)

- **Server state**: TanStack Query v5 (Q-DECISION-4). Conventions:
  - Query keys: `['cirs', 'lookup', { method, value }]`, `['wallet', customer_id]`, `['sessions', 'active']`, etc.
  - StaleTime: 0 (fresh on every mount) for billing-critical; 60s for menus + reference data
  - Refetch interval: 5s for active sessions; 10s for system status; off for menus
  - Mutation keys: `['wallet', 'topup']`, `['cafe', 'order']`, etc.
- **Client state**: useReducer for screen-local state machines (e.g. lookup screen). useState for trivial UI state. Context only for staff-session (one provider at root).
- **URL state**: Next.js searchParams + useSearchParams for filter state, query parameters; ALWAYS canonical (canonicalized phone in URL after lookup, etc.).

### §5.6 Error states (UNIVERSAL across POS screens)

| Error | Render | Recovery |
|---|---|---|
| Network unreachable | red banner top-of-content; retry button | Auto-retry every 30s; manual retry button |
| 401 (session expired) | redirect to `/v2/pos/login?return=<current_url>` | Re-auth restores |
| 403 (insufficient role) | red banner "This action requires manager role" | No recovery; manager intervention |
| 404 (resource gone) | empty-state "Customer not found" / "Order not found" | Back to previous screen |
| 409 (concurrent conflict) | red banner "Another session is editing this customer; refresh and retry" | Manual refresh |
| 500 (server error) | red banner "Server error — try again or notify Captain"; error_id displayed for support | Manual retry |
| Validation error (400) | inline field error; field highlighted red; submit disabled | Edit fields and resubmit |

---

## §6 — Component Reuse Matrix (Layer 1 → Layers 2-7)

This matrix signals which Layer 1 components feed Layer 2-7 design — the design agent should author them with reuse in mind.

| Layer 1 component | Layer 2 PWA | Layer 3 .23 portal | Layer 4 pod display | Layer 5 kiosk + chef | Layer 6 admin | Layer 7 marketing |
|---|---|---|---|---|---|---|
| `<Button>` size+variant | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `<Input>` | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ |
| `<NumPad>` | ✓ (PWA OTP) | ✗ | ✗ | ✓ (kiosk) | ✗ | ✗ |
| `<Card>` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `<Modal>` | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ |
| `<Badge>` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `<Toast>` | ✓ | ✓ | ✗ | ✓ | ✓ | ✓ |
| `<Spinner>` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `<EmptyState>` | ✓ | ✓ | ✓ | ✓ | ✓ | ✗ |
| `<ErrorBoundary>` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| `<Money>` | ✓ wallet view | ✓ active session | ✗ | ✓ chef order total | ✓ admin reports | ✗ |
| `<Phone>` | ✓ profile | ✓ portal | ✗ | ✓ chef contact | ✓ admin search | ✗ |
| `<DateTime>` | ✓ history | ✓ session-start | ✓ paused-since | ✓ order-time | ✓ all reports | ✗ |
| `<StaffPinGate>` | ✗ | ✓ | ✗ | ✓ chef | ✓ | ✗ |
| `<ProfilePreviewCard>` | ✓ self-view | ✓ pre-launch confirm | ✗ | ✗ | ✓ admin search | ✗ |
| `<WalletBalanceCard>` | ✓ wallet view | ✓ active session sidebar | ✗ | ✗ | ✓ customer detail | ✗ |
| `<WalkInGuestDropdown>` | ✗ | ✓ | ✗ | ✗ | ✗ | ✗ |
| `<PaymentMethodSelector>` | ✓ V2.1 self-topup | ✗ | ✗ | ✗ | ✓ refund | ✗ |
| `<ReceiptCard>` | ✓ history | ✗ | ✗ | ✗ | ✓ reprint | ✗ |
| `<CafeOrderRow>` | ✗ | ✗ | ✗ | ✓ chef display | ✓ admin reports | ✗ |
| `<TierLadderTable>` | ✓ V2.1 self-topup | ✓ pre-launch estimate | ✗ | ✗ | ✓ pricing editor | ✓ marketing site |
| `<SessionRow>` | ✓ history | ✓ active list | ✗ | ✗ | ✓ admin reports | ✗ |

**Implication for design agent**: design these components as standalone, fully-self-contained primitives with TypeScript types exported. Each subsequent layer imports them rather than re-creating.

---

## §7 — Subsequent Layer Skeletons (one-line scope per layer; for cross-layer planning only)

These are NOT in Layer 1 scope — they are signaled here so the design agent understands how Layer 1 fits the larger picture.

### Layer 2 — PWA customer surface (`app.racingpoint.cloud`)
- 375px mobile primary; tablet 768px deferred V2.1
- Routes: `/register` (phone+OTP), `/profile` (multi-profile management), `/wallet` (balance + history), `/history` (session history)
- V2.0 customer self-serve = registration ONLY; ALL other actions are staff-driven via POS .130
- Inherits Foundation §3 + reuses `<ProfilePreviewCard>`, `<WalletBalanceCard>`, `<TierLadderTable>` from Layer 1

### Layer 3 — Server .23 launch portal (`192.168.31.23/portal`)
- 1024px+ responsive; staff surface
- Routes: `/portal/customers` (auto-appearance from CIRS), `/portal/launch` (preset+pod+game), `/portal/sessions/active`, `/portal/lobbies`, `/portal/pause/[session_id]`
- AC Multiplayer dedicated server management (VMS pattern)
- Inherits Foundation + reuses `<ProfilePreviewCard>`, `<SessionRow>`, `<StaffPinGate>` from Layer 1

### Layer 4 — Pod display "Session Paused" + state surfaces
- 1280×720 fixed (per pod)
- Customer-visible state ONLY (CR-2: pod is for customer-facing state, not .23)
- Surfaces: paused screen, balance-runout warning, end-of-session message, audio-cue trigger surface (F29 invalid-lap)
- Inherits Foundation; minimal component reuse (display-only)

### Layer 5 — Kiosk (audible balance-runout) + Chef display + WhatsApp ordering
- Kiosk: 1280×1024 staff-visible; audio alarm + 2-min grace timer
- Chef display: 1920×1080 fixed; queue of cafe orders by station; audio + visual cues
- WhatsApp ordering: NOT a Next.js surface — message templates + chatbot flows (separate handoff)
- Inherits Foundation; reuses `<CafeOrderRow>` from Layer 1

### Layer 6 — Admin panel (pricing editor / pending payment / billing module)
- 1280px+ responsive; admin role required
- Routes: `/admin/pricing` (Way A tier editor per §AMEND-3.II admin-adjustable); `/admin/pending-payments` (full-fleet view); `/admin/billing-transactions` (search + reports); `/admin/customers` (search + detail); `/admin/credit-ledger` (apology + adjustments)
- Inherits Foundation + reuses Layer 1 admin-class primitives

### Layer 7 — Marketing site + WhatsApp templates
- racingpoint.in marketing site (V2.0 SEO + brand)
- WhatsApp re-engagement templates ("your unused credits are waiting")
- Inherits Foundation brand only; NOT a Next.js app per the same monorepo (separate handoff)

---

## §8 — Open Captain decisions BEFORE design starts

Design agent: Captain must answer these before authoritative design output. Layer 1 default-leans noted; design agent may proceed on defaults if Captain pre-authorizes.

| ID | Decision | Layer 1 lean | Why deferred to Captain |
|---|---|---|---|
| Q-DECISION-1 | Enthocentric font sourcing | (b) Montserrat 700 substitute | License unclear; smallest reversible |
| Q-DECISION-2 | POS responsive: 1920×1080 fixed only OR also support staff laptop fallback (1280×800)? | Fixed 1920×1080 only | Hardware fixed in V2.0; kaizen smallest scope |
| Q-DECISION-3 | Forms library: React Hook Form + Zod OR plain controlled components? | RHF + Zod | Industry standard; less custom code |
| Q-DECISION-4 | Server state: TanStack Query v5 OR plain fetch + useEffect? | TanStack Query v5 | Polling, cache, mutations baked in |
| Q-DECISION-5 | Staff PIN lockout policy: lockout after N fails? | Captain to define (likely 5 fails → 5min lockout); design accommodates either way | Security-debt-ledger consideration |
| Q-DECISION-6 | Staff PIN re-entry per CIRS lookup OR session-cookie sufficient? | Session-cookie sufficient (PIN at session start; PIN re-entry only on privileged actions like manager-role apology credits) | Per PACT-20260506-001 §7 Q5 — bono AMPLIFIER input requested |
| Q-DECISION-7 | POS-local cache layer for ProfilePreview? | Q2-C no-cache (per PACT-20260506-001 §7 Q2 default) | Substrate query <50ms; cache layer = Phase 2 |
| Q-DECISION-8 | Customer registration capture: include Aadhaar / GST identifier? | NO for V2.0 (V2.1 scope) | DPDP-compliance + consent flow needed |
| Q-DECISION-9 | Discount surface: happy-hour + iRacing 20% (Tier-2 missed scenarios H/I) | Captain to choose auto-apply vs staff-toggle | Memory says auto-apply; not yet locked |
| Q-DECISION-10 | Receipt printing: browser print dialog OR thermal-printer protocol (Print Module sub-PACT) | Browser print for V2.0; Print Module = sub-PACT later | Q-PRINT-1/2/3 still open per §S-57 |
| Q-DECISION-11 | Logo asset: provided SVG OR design agent drafts? | Captain provides if exists; otherwise design agent drafts based on brand identity | Asset inventory: no `web-v2/public/` yet |

**Captain shortcut**: ratify all defaults at once with "ratify Q-DECISION defaults" — design agent proceeds on the lean column.

---

## §9 — Anti-patterns explicit (DO NOT DO)

These are common failure modes for AI-generated frontend design. Each is a tripwire — if the design output exhibits ANY of these, regenerate with this section emphasized.

### Brand / visual

1. **DO NOT use shadcn/ui or any component library** — project convention is utility-first Tailwind 4 + inline SVG. shadcn-look-alike (rounded-md cards on slate-900 bg with blue accents) is the SaaS-AI default and does NOT match Racing Point brand.
2. **DO NOT use Lucide / react-icons / heroicons** — inline SVG only.
3. **DO NOT use orange `#FF4400`** — explicitly deprecated V1 brand orange.
4. **DO NOT default to dark-mode toggle** — Racing Point IS dark-mode. There is no light mode in V2.0.
5. **DO NOT use generic "Material You" / "iOS-style" palettes** — brand is Racing Red on Asphalt Black with Gunmetal Grey neutrals. Period.
6. **DO NOT add gradient backgrounds, glassmorphism, neumorphism, claymorphism** — flat design only. Single-tone backgrounds.
7. **DO NOT add purple, teal, pastel** — anything not in §3.1 brand palette is wrong.
8. **DO NOT use system fonts as primary** — Montserrat (body) + Enthocentric/Montserrat-700 (headers). System fonts are FALLBACK only.

### Motion / animation

9. **DO NOT use framer-motion or any animation library** — Tailwind transitions only.
10. **DO NOT add `animate-spin`, `animate-pulse`, `animate-bounce`** anywhere on POS — distraction = slowdown.
11. **DO NOT add micro-interactions on hover (scale-105, rotate, etc.)** — POS staff don't need them; they create false-positive "is it loading?" perceptions.
12. **DO NOT add page-transition animations** — instant route changes only.

### Layout / responsive

13. **DO NOT design POS responsive (375/768/1024/1440 breakpoints)** — POS .130 is FIXED 1920×1080. Layer 2 PWA is 375px. Mixing breakpoint logic into POS adds dead code.
14. **DO NOT add a mobile menu / hamburger to POS** — staff workflow is NOT mobile.
15. **DO NOT design two-column layouts that collapse to single-column** on POS — fixed 1920×1080 means two-column STAYS two-column.

### Tech stack

16. **DO NOT install shadcn-ui via CLI** — even if "convention says shadcn for forms". Project convention overrides defaults.
17. **DO NOT install `@radix-ui` primitives** — same reason. Build modals + dropdowns + tooltips with primitive Tailwind + `<dialog>` element + headless React.
18. **DO NOT install global state (Zustand/Jotai/Redux)** — useReducer + Context is enough for Layer 1.
19. **DO NOT install `axios`** — `fetch` + TanStack Query is sufficient.
20. **DO NOT install `clsx` or `cva`** — Tailwind 4 has CSS variables for variants; if needed, use `class:` array template.

### State / API

21. **DO NOT mock backend data inline** — every API call is real (substrate is SHIPPED on origin/main). If endpoint doesn't exist yet, leave a `TODO: Phase 1 wire-up — POST /api/v1/cirs/lookup`. NEVER hardcode mock customer "John Doe" in a component.
22. **DO NOT use `useState({ ... mockData })`** — server state belongs in TanStack Query.
23. **DO NOT swallow errors silently** — every `catch` either renders an error UI OR re-throws. CR-3 customer-service-priority demands visible failures.
24. **DO NOT use synthetic IDs** — UUIDs come from server; client-side generation = drift.

### Money / billing

25. **DO NOT use floats for money** — paise (i64) end-to-end. Display via `<Money>` primitive.
26. **DO NOT compute GST on client** — server returns `gst_collected_paise`. Client renders.
27. **DO NOT compute tier-ladder breakdown on client** — server returns breakdown via `/api/v1/billing/estimate`. Client renders.
28. **DO NOT generate tax_invoice_no on client** — server-side per WalletTopup.
29. **DO NOT show "use credits" anywhere on cafe surface** — CR-5 hard rule.
30. **DO NOT round money for display** — show paise-precision when needed (₹X.YZ); never strip to integer rupees.

### Audit / privacy

31. **DO NOT log phone numbers in console** — DPDP compliance + audit-trail says SHA256-only logging. Components log `customer_id` (UUID), never phone.
32. **DO NOT include PIN in any error message, log, telemetry, or component prop** — even hashed.
33. **DO NOT include credit card numbers / UPI VPA in DOM after submit** — payment_ref is OK for last-4 / txn-id only.
34. **DO NOT add "remember me" or "save card"** — V2.0 staff sessions are ephemeral.

### Accessibility

35. **DO NOT use color-only signals** — every status color also has an icon (✓ / ✕ / ⚠).
36. **DO NOT skip aria-label on icon-only buttons** — POS staff may use voice-readers in the future.
37. **DO NOT trap focus outside modals** — modal opens, focus enters; modal closes, focus returns to trigger.
38. **DO NOT auto-focus on page mount for non-form pages** — disorienting.
39. **DO NOT use `<div onClick>` for buttons** — `<button>` element only.

### Workflow / UX

40. **DO NOT design "back" buttons that route to dashboard for every screen** — back should mean PREVIOUS screen in workflow (use Next.js router.back() with fallback to dashboard).
41. **DO NOT design "are you sure?" confirms for low-risk reversible actions** — reserve confirms for irreversible (mark-paid, end-session, issue-apology-credit, register-customer).
42. **DO NOT design 4+ step wizards in V2.0** — staff prefer single-screen forms; multi-step adds friction.
43. **DO NOT design self-serve customer paths** — V2.0 is staff-driven (CR-1).
44. **DO NOT auto-redirect after delays** — staff need control. Explicit CTAs only.
45. **DO NOT show toasts for happy-path success when the screen already changed** — redundant. Toast = "background event you should know about", not "thing you just did".

---

## §10 — Acceptance criteria + UAT

### §10.1 Per-screen acceptance

See §5.3 for screen-by-screen acceptance lists. Each screen has a checklist; design agent should reproduce the checklist in the design output and mark testable criteria.

### §10.2 Cross-cutting acceptance (Layer 1 as a whole)

- [ ] Staff can complete Scenario 1 (new-customer-alone) end-to-end at POS in <90s: lookup-not-found → register → top-up → return to dashboard
- [ ] Staff can complete Scenario A (returning customer) in <30s: lookup-found → top-up → done
- [ ] Cafe order with optional phone capture in <45s
- [ ] Walk-In Guest fallback completes in <20s
- [ ] All actions auditable: every CIRS lookup, every wallet event, every cafe order writes to audit table (verify via server log; UI does not need to surface)
- [ ] No visual flicker on TanStack Query refetch (active sessions polling every 5s)
- [ ] Print receipt works on browser print dialog (Q-DECISION-10 default)
- [ ] Tab-order navigates input → numpad → submit logically
- [ ] All money in paise; all money displays via `<Money>` primitive; audit grep `(?<!font-mono)` next to amount string = 0 matches
- [ ] All datetime UTC ↔ IST converted via `<DateTime>` primitive; audit grep for `new Date()` outside primitive = 0 matches
- [ ] No phone strings in DOM/console outside `<Phone>` primitive
- [ ] WCAG AA on all screens (axe-core scan)
- [ ] Lighthouse: Performance ≥ 90 (POS-fixed-resolution simplifies); Accessibility ≥ 95; Best Practices ≥ 95

### §10.3 Pre-ship gate (per CLAUDE.md Standing Rules)

- [ ] `gsd-ui-researcher` produced UI-SPEC.md (this doc satisfies that for Layer 1)
- [ ] `gsd-ui-auditor` 6-pillar visual audit on rendered output (after design ships)
- [ ] Quality Gate: `bash test/run-all.sh` passes
- [ ] E2E: Playwright tests for Scenario 1 + Scenario A + cafe order
- [ ] Multi-Model Audit (MMA) on cross-system bridges (CIRS + wallet integration = MMA mandatory per CLAUDE.md)
- [ ] DEPLOY PARITY: web-v2 deployed to Server .23 :3500 + Bono VPS :3500 + verified
- [ ] DMP audit (`bash scripts/deploy/deploy-audit.sh`)
- [ ] Self-audit: `bash tests/page-audit/self-audit.sh` baseline + post-change

---

## §11 — References

### Authoritative sources (READ BEFORE DESIGNING)

- `racecontrol/CLAUDE.md` — Brand Identity LOCKED; Standing Rules; Network Map
- `~/.claude/projects/C--Users-bono/memory/project_v2_customer_workflows_consolidated_20260503.md` — 5 base + 6 missed scenarios; 30-feature V2.0 list
- `~/.claude/projects/C--Users-bono/memory/session_handoff_20260506_v2_customer_billing_workflow_consolidated_PRIMARY.md` — §AMEND-1 → §AMEND-4.H locks (Way A pricing, MI deferral, F29 audio cue, etc.)
- `racecontrol/.planning/phases/91-session-experience/91-UI-SPEC.md` — Phase 91 design system inheritance source (Tailwind 4, NO shadcn, inline SVG, Montserrat 400/700)
- `racecontrol/pwa/src/app/globals.css` — existing PWA design tokens (rp-* @theme block) — duplicate-not-import in web-v2 per quarantine-discipline
- `racecontrol/web-v2/` — Next.js 16.1.6 scaffold (current state; design adds to this)
- `racecontrol/crates/v2-db/src/cirs.rs` — CIRS API canonical contract
- `racecontrol/crates/v2-db/src/wallets.rs` — Wallet API canonical contract
- `racecontrol/crates/v2-db/src/customers.rs` — Customer API canonical contract
- `racecontrol/comms-link/proposals/PACT-20260505-001-v2-identity-primitive-customer-resolution.md` — RATIFIED Phase 0 PACT
- `racecontrol/comms-link/proposals/PACT-20260506-001-pact001-phase-1-wireup.md` — DRAFT Phase 1 PACT (the implementation companion to this design handoff)

### Useful patterns (READ FOR INSPIRATION)

- `racecontrol/pwa/src/app/` — existing 21 PWA routes following the convention (mobile, but design tokens apply)
- `racecontrol/.planning/phases/35-credits-ui/35-UI-SPEC.md`
- `racecontrol/.planning/phases/82-billing-and-session-lifecycle/82-UI-SPEC.md`

### What to ASK Captain about (open Q-DECISION-1..11 — see §8)

When in doubt: defer to Captain via the Q-DECISION ID system, not silent invention.

---

## §12 — Design agent invocation guidance

When this handoff is shipped to the design agent (`frontend-design` skill / `ui-ux-pro-max` skill / `gsd-ui-researcher` subagent):

1. **Pre-load context**: read this entire doc + the substrate files (cirs.rs, wallets.rs) + Phase 91 UI-SPEC + pwa globals.css.
2. **Resolve Q-DECISION blockers first**: if Captain has not pre-authorized defaults, halt and ask. If pre-authorized, proceed on lean column.
3. **Author Foundation FIRST**: §3 design tokens + globals.css + component primitives §3.7. NO POS surface design until foundation is in place.
4. **Author POS surfaces SECOND**: per §5.3 in route order (login → dashboard → lookup → register → topup → cafe → billing → session detail).
5. **Test against §9 anti-patterns**: re-read §9 BEFORE marking design complete; grep output for known anti-pattern signals (shadcn imports, Lucide imports, orange #FF4400, framer-motion, etc.).
6. **Output structure**:
   - `web-v2/src/app/globals.css` (extended with §3.9)
   - `web-v2/src/components/` (primitives + composites; one file per component)
   - `web-v2/src/components/icons/` (inline SVG components)
   - `web-v2/src/app/pos/` (route segments per §5.2)
   - `web-v2/tests/` (component tests + E2E)
   - Per-component `.stories.tsx` if Storybook is added (Q-DECISION not asked — design agent may propose)
7. **Acceptance verification**: against §10 checklist; produce a UI-REVIEW.md per `gsd-ui-auditor` convention.
8. **Iteration discipline**: if Captain feedback corrects a brand/anti-pattern violation, regenerate the affected component with §9 emphasized.

---

— james / 2026-05-06 ~16:30 IST · V2 Design Handoff Layer 1 — Foundation + POS .130 Staff Terminal · 1100+ lines · ready for Captain review before ship to design agent · subsequent layers (2-7) inherit Foundation §3 + reuse Layer 1 component primitives per §6 reuse matrix
