# Requirements: v43.0 Self-Audit & Visual Regression System

**Defined:** 2026-04-06
**Core Value:** James autonomously verifies all frontend pages before/after fixes -- eliminating blind code-only fixes.

## v43.0 Requirements

Requirements for visual regression and self-audit system. Each maps to roadmap phases.

### Page Crawler

- [x] **CRAWL-01**: Script visits all pages across web (:3200), admin (:3201), and kiosk (:3300) and captures full-page screenshots
- [x] **CRAWL-02**: Script authenticates via saved staff PIN state (Playwright storageState)
- [x] **CRAWL-03**: Screenshots saved to structured directory: `tests/screenshots/{app}/{route}/{timestamp}.png`
- [x] **CRAWL-04**: Script can target specific apps or pages (not always full crawl)

### Visual Regression

- [ ] **VR-01**: Playwright toHaveScreenshot() tests for critical pages with baseline comparison
- [ ] **VR-02**: Dynamic content masking (timestamps, counters, live metrics) per-page configuration
- [ ] **VR-03**: Baselines stored in git alongside test files
- [ ] **VR-04**: Before/after screenshot capture integrated into frontend fix workflow

### Enforcement Hooks

- [ ] **HOOK-01**: Claude Code hook blocks "fixed/done/resolved" claims for frontend changes unless screenshot evidence exists
- [ ] **HOOK-02**: Hook only triggers for frontend-related changes (Next.js, CSS, React) -- not Rust backend or scripts
- [ ] **HOOK-03**: Hook checks for screenshot file newer than last code edit in session

### Deploy Integration

- [ ] **DEPLOY-01**: Page crawler auto-runs after deploy-nextjs.sh completes
- [ ] **DEPLOY-02**: Build hash verification table showing expected vs running build on all targets
- [ ] **DEPLOY-03**: Deploy script exits with failure if page crawler finds visual regressions

### AI Self-Audit

- [ ] **AUDIT-01**: Page description files documenting expected behavior per page (what data, what layout, what interactions)
- [ ] **AUDIT-02**: James reads fresh screenshots via Read tool and compares against descriptions
- [ ] **AUDIT-03**: Anomaly report generated listing pages that don't match expected behavior
- [ ] **AUDIT-04**: Self-audit runs at session start when working on frontend tasks

## Future Requirements

### Extended Coverage

- **EXT-01**: PWA customer app crawling (requires OTP test account)
- **EXT-02**: Cloud endpoint visual regression (Bono VPS URLs)
- **EXT-03**: Cross-browser testing (Chrome, Edge, Firefox)
- **EXT-04**: Mobile viewport screenshots for PWA
- **EXT-05**: Performance baseline tracking (page load times)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Cloud visual testing services (Percy, Chromatic) | Overkill for single-developer, adds external dependency |
| BackstopJS | Redundant -- Playwright already installed with same capabilities |
| Cross-browser rendering comparison | All targets use same Chrome/Edge engine |
| Video recording of page interactions | Screenshots sufficient for static verification |
| Automated bug fixing based on visual diffs | AI identifies issues, human/James fixes them |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CRAWL-01 | Phase 325 | Complete |
| CRAWL-02 | Phase 325 | Complete |
| CRAWL-03 | Phase 325 | Complete |
| CRAWL-04 | Phase 325 | Complete |
| VR-01 | Phase 326 | Pending |
| VR-02 | Phase 326 | Pending |
| VR-03 | Phase 326 | Pending |
| VR-04 | Phase 326 | Pending |
| HOOK-01 | Phase 327 | Pending |
| HOOK-02 | Phase 327 | Pending |
| HOOK-03 | Phase 327 | Pending |
| DEPLOY-01 | Phase 327 | Pending |
| DEPLOY-02 | Phase 327 | Pending |
| DEPLOY-03 | Phase 327 | Pending |
| AUDIT-01 | Phase 328 | Pending |
| AUDIT-02 | Phase 328 | Pending |
| AUDIT-03 | Phase 328 | Pending |
| AUDIT-04 | Phase 328 | Pending |

**Coverage:**
- v43.0 requirements: 17 total
- Mapped to phases: 17
- Unmapped: 0

---
*Requirements defined: 2026-04-06*
*Last updated: 2026-04-06 after roadmap creation (all 17 requirements mapped to phases 325-328)*
