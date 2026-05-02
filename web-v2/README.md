# web-v2 — RacingPoint V2 dedicated Next.js host

## Status

**Phase 0.1 SUBSTRATE — authored 2026-05-03 under PACT-20260503-001 substrate-ship-autonomous (Z2 authoring).**

Current state: scaffold only. NOT installed (no `npm install`), NOT built, NOT running, NOT registered with PM2, NOT routed via nginx. Activation gates on bono AMPLIFIER vote landing + Captain Level B ratify.

## Purpose

Dedicated Next.js host for the V2 RacingPoint ecosystem. Severs S3 SEAM (shared host coupling) and S4 SEAM (forward-DB coupling to V1) per PACT-015 R1 Phase 0 + composite-ratify-event #3 D2/D3.

- **Port:** `:3500` (Captain D3 confirmed; Bono VPS `:3400` occupied by `racingpoint-dashboard` pm2 id=12 pid=3624929)
- **basePath:** `/v2` (V2 surfaces serve under `:3500/v2/*`)
- **Build mode:** standalone (F9 Atomic-Deploy compatible)
- **Health probe:** `:3500/v2/api/v1/health` (V2-namespaced; doesn't collide with V1 `:3200/api/v1/health`)

## Quarantine discipline (§2 of PACT-20260503-001)

V2 host stand-up is **purely additive**. No V1 substrate writes:
- No edits to `web/server.js` / `web/next.config.ts` / `web/src/app/globals.css`
- No port collision with V1 web (`:3200`) or admin (`:3201`) or kiosk (`:3300`)
- No relative-path imports into `../packages/shared-tokens/` (avoids the V1 Turbopack blocker)

Shared-tokens integration is **deferred** to Phase 0.2+ via NPM workspace pattern (not Turbopack relative-path import). Until then, this host uses its own minimal CSS.

## Scope (§5 of PACT-20260503-001 — what this scaffold does NOT include)

- F9 Atomic-Deploy implementation (Phase 0.3 child-PACT)
- F2 Feature Flag Service (Phase 0.4 child-PACT)
- V2-native database (Phase 0.2 child-PACT — SQLite-first per Captain D4)
- JWT bilateral sweep (Phase 0.5 child-PACT — Captain D6)
- V2-native deployment pipeline (Phase 0.7 child-PACT)
- V1-quarantine-runtime-filter hooks (Phase 0.9 child-PACT — Captain D8)
- The 7 shipped V2 surfaces (Kiosk + Admin batch + POS Customer Detail + PWA Lap Compare + Flag Hub) — these stay in `kiosk/` + `web/` until activation phase routes them under `:3500/v2/*`

This Phase 0.1 substrate provides the **host shell + health probe**. The actual surface migration is the activation step that gates on bono AMPLIFIER + Captain Level B ratify.

## Activation gates (§9 of PACT-20260503-001)

Before this host runs in any environment:
1. bono AMPLIFIER vote — answers Q1-Q5 (pm2 path / nginx routing / cloud subdomain vs path / health-probe shape / F9+F2 install timing)
2. Captain Level B ratify (D2 inheritance auto-ratify path or explicit verb)
3. Caveat absorption per CONSTRAINT-009 5-row checklist
4. EXECUTE: `npm install` + `npm run build` + PM2 register + nginx routing + venue parity
5. VERIFY: §8 6-criteria (process running on `:3500` + 7 surfaces accessible + V1 unaffected + health 200 OK + 30min pm2 soak + no V1 writes)

## File tree

```
web-v2/
├── README.md               (this file)
├── package.json            (Next.js + React deps; matches web/ versions)
├── next.config.ts          (port 3500 / basePath /v2 / standalone / outputFileTracingRoot)
├── tsconfig.json           (TypeScript config)
├── .gitignore              (node_modules, .next/)
└── src/
    └── app/
        ├── layout.tsx      (root layout)
        ├── page.tsx        (Phase 0.1 placeholder homepage)
        ├── globals.css     (own minimal CSS; NO shared-tokens import)
        └── api/
            └── v1/
                └── health/
                    └── route.ts   (health probe — returns 200 + JSON)
```

## Composes-with

- PACT-013 V2 ROLLOUT META Phase A (substrate completion)
- PACT-015 R1 Phase 0 sub-step 0.1 + R6 V2 Contract Dependency Graph
- PACT-018 Phase 2 OpenRouter Phase A pre-req (operational track)
- V2-FOUNDATION-MILESTONE.md
- v2-skeleton/01-skeleton-architecture.md

## See also

- Canonical proposal: `comms-link/proposals/PACT-20260503-001-phase-0-1-v2-host-standup-3500.md`
- INBOX HANDOFF: comms-link `INBOX.md` 2026-05-03 02:05 IST entry (msg id=34744)
- Captain forward-execution verb: AUTHORIZE 2026-05-03 — *"Phase 0.1 first, then Scenario 1 vertical slice. ... Proceed autonomously."*
