// Layer 1 Foundation demo page — exercises the design system tokens
// from Captain claude.ai/design bundle 2026-05-02 (v2-design-h-V3XSuJJ)
// per CAPTAIN-RATIFIED §3 of V2-DESIGN-LAYER-1-POS-FOUNDATION.md.
//
// Not a final POS surface — placeholder until Layer 1 component
// authoring + POS routes scaffold in subsequent passes.

export default function Home() {
  return (
    <main className="mx-auto max-w-3xl px-6 py-16 space-y-8">
      <header className="space-y-3">
        <p className="font-body uppercase tracking-[0.08em] text-xs font-semibold text-rp-grey">
          Phase 0.1 · Layer 1 Foundation
        </p>
        <h1 className="font-display font-bold text-5xl tracking-tight text-rp-ink leading-none">
          RacingPoint <span className="text-rp-red">V2</span>
        </h1>
        <p className="font-body text-base text-rp-ink-dim leading-relaxed max-w-xl">
          V2 host scaffold under PACT-20260503-001. Foundation design tokens
          ratified from Captain claude.ai/design bundle 2026-05-02 (v2-design-h-V3XSuJJ).
        </p>
      </header>

      <section className="space-y-3">
        <h2 className="font-display font-semibold uppercase tracking-[0.04em] text-lg text-rp-ink">
          Brand
        </h2>
        <div className="grid grid-cols-3 gap-2">
          <Swatch token="--color-rp-red" hex="#E10600" label="rp-red" />
          <Swatch token="--color-rp-red-deep" hex="#B40500" label="rp-red-deep" />
          <Swatch token="--color-rp-red-glow" hex="#FF1A0E" label="rp-red-glow" />
        </div>
      </section>

      <section className="space-y-3">
        <h2 className="font-display font-semibold uppercase tracking-[0.04em] text-lg text-rp-ink">
          Surfaces
        </h2>
        <div className="grid grid-cols-3 gap-2">
          <Swatch token="--color-rp-asphalt" hex="#0D0D0D" label="asphalt" />
          <Swatch token="--color-rp-dark" hex="#1A1A1A" label="dark" />
          <Swatch token="--color-rp-card" hex="#1C1C1C" label="card" />
          <Swatch token="--color-rp-card-hi" hex="#262626" label="card-hi" />
          <Swatch token="--color-rp-border" hex="#2A2A2A" label="border" />
          <Swatch token="--color-rp-border-hi" hex="#3A3A3A" label="border-hi" />
        </div>
      </section>

      <section className="space-y-3">
        <h2 className="font-display font-semibold uppercase tracking-[0.04em] text-lg text-rp-ink">
          Semantic
        </h2>
        <div className="grid grid-cols-3 gap-2">
          <Swatch token="--color-rp-green" hex="#00D26A" label="green" />
          <Swatch token="--color-rp-amber" hex="#FFB000" label="amber" />
          <Swatch token="--color-rp-blue" hex="#3B82F6" label="blue" />
        </div>
      </section>

      <section className="space-y-3">
        <h2 className="font-display font-semibold uppercase tracking-[0.04em] text-lg text-rp-ink">
          Typography
        </h2>
        <div className="space-y-2 rounded-md border border-rp-border bg-rp-card p-4">
          <p className="font-display font-bold text-4xl text-rp-ink">Chakra Petch · Display</p>
          <p className="font-body text-base text-rp-ink-dim">Montserrat · Body 14/1.5 — fast staff feedback</p>
          <p className="font-mono text-sm text-rp-amber">JetBrains Mono · 12:34.567 · ₹2,700.00</p>
        </div>
      </section>

      <footer className="pt-6 border-t border-rp-border">
        <a
          className="inline-flex items-center gap-2 font-mono text-sm font-medium text-rp-red hover:text-rp-red-glow transition-colors"
          href="/v2/api/v1/health"
        >
          → Health probe (PACT-20260503-001)
        </a>
      </footer>
    </main>
  );
}

function Swatch({ token, hex, label }: { token: string; hex: string; label: string }) {
  return (
    <div className="rounded-md border border-rp-border overflow-hidden">
      <div className="h-12" style={{ background: `var(${token})` }} />
      <div className="px-3 py-2 bg-rp-card">
        <p className="font-mono text-[11px] text-rp-ink uppercase tracking-wider">{label}</p>
        <p className="font-mono text-[10px] text-rp-ink-dim">{hex}</p>
      </div>
    </div>
  );
}
