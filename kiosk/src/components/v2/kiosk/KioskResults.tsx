'use client';

// Racing Point V2 Kiosk — Results (podium + class progress + QR-to-PWA)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { PrimaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';

export function KioskResults({ onLock }: { onLock: () => void }) {
  return (
    <KioskFrame podId="POD-01" state="idle">
      <div style={{ height: '100%', display: 'flex', flexDirection: 'column', padding: '24px 40px', overflow: 'auto' }}>
        <div style={{ ...t('caption'), fontSize: 11, color: RP.red }}>● SESSION COMPLETE · UDAY SINGH</div>

        <div style={{ display: 'grid', gridTemplateColumns: '1.2fr 1fr', gap: 20, marginTop: 14, flex: 1, minHeight: 0 }}>
          <div>
            <div style={{ ...t('displayM'), fontFamily: FONT.display, fontSize: 40, color: RP.ink, lineHeight: 1 }}>P1</div>
            <div style={{ ...t('h2'), fontSize: 16, color: RP.inkDim, marginTop: 4, fontWeight: 400 }}>Nice drive.</div>

            <div style={{
              marginTop: 18, padding: '20px 24px', background: RP.base,
              border: `1px solid ${RP.border}`, borderLeft: `3px solid ${RP.red}`,
            }}>
              <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>FASTEST LAP · ON LAP 7 OF 9</div>
              <div style={{ ...t('mono'), fontSize: 48, color: RP.ink, marginTop: 4, letterSpacing: '0.02em' }}>2:00.630</div>
              <div style={{ ...t('bodySm'), fontSize: 11, color: RP.green, marginTop: 4 }}>▲ 0.482s personal best</div>
            </div>

            <div style={{ marginTop: 14, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 8 }}>
              <ResultStat label="LAPS" v="9" />
              <ResultStat label="AVG LAP" v="2:03.418" />
              <ResultStat label="TOP SPEED" v="287" sub="km/h" />
              <ResultStat label="CONSISTENCY" v="94%" />
            </div>

            <div style={{
              marginTop: 14, padding: '14px 18px',
              background: RP.base, border: `1px solid ${RP.border}`,
            }}>
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
                <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>DRIVER CLASS</div>
                <div style={{ ...t('caption'), fontSize: 10, color: RP.green }}>+12 PTS</div>
              </div>
              <div style={{ display: 'flex', alignItems: 'baseline', gap: 12, marginTop: 4 }}>
                <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 18, color: RP.classApex }}>APEX</div>
                <div style={{ ...t('mono'), fontSize: 11, color: RP.inkDim }}>72 / 100 to PODIUM</div>
              </div>
              <div style={{ height: 6, background: RP.border, marginTop: 8, position: 'relative' }}>
                <div style={{
                  position: 'absolute', left: 0, top: 0, height: '100%', width: '72%',
                  background: RP.classApex,
                }} />
              </div>
            </div>
          </div>

          <div style={{ display: 'flex', flexDirection: 'column' }}>
            <div style={{ padding: '16px 20px', background: RP.card, border: `1px solid ${RP.border}` }}>
              <div style={{ ...t('caption'), fontSize: 10, color: RP.gunmetal }}>SEE YOUR TELEMETRY</div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginTop: 10 }}>
                <div style={{ width: 84, height: 84, background: RP.ink, padding: 6, flexShrink: 0 }}>
                  <div style={{
                    width: '100%', height: '100%',
                    backgroundImage: `linear-gradient(45deg, ${RP.asphalt} 25%, transparent 25%, transparent 75%, ${RP.asphalt} 75%), linear-gradient(45deg, ${RP.asphalt} 25%, transparent 25%, transparent 75%, ${RP.asphalt} 75%)`,
                    backgroundSize: '8px 8px',
                    backgroundPosition: '0 0, 4px 4px',
                  }} />
                </div>
                <div>
                  <div style={{ ...t('h2'), fontSize: 14, color: RP.ink }}>Scan with phone</div>
                  <div style={{ ...t('bodySm'), fontSize: 11, color: RP.inkDim, marginTop: 4, lineHeight: 1.5 }}>
                    Opens this lap in the Racing Point app — throttle, brake, sector breakdown, share to WhatsApp.
                  </div>
                </div>
              </div>
            </div>

            <div style={{ flex: 1 }} />

            <div style={{
              padding: '14px 18px', background: RP.base, border: `1px dashed ${RP.amber}`,
              ...t('mono'), fontSize: 10, color: RP.amber,
            }}>
              ● STAFF ONLY · LOCK POD WHEN DRIVER LEAVES
            </div>
            <div style={{ marginTop: 12, display: 'flex', justifyContent: 'flex-end' }}>
              <PrimaryCTA onClick={onLock} shortcut="ENTER">Lock pod →</PrimaryCTA>
            </div>
          </div>
        </div>
      </div>
    </KioskFrame>
  );
}

function ResultStat({ label, v, sub }: { label: string; v: string; sub?: string }) {
  return (
    <div style={{ padding: '10px 12px', background: RP.base, border: `1px solid ${RP.border}` }}>
      <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>{label}</div>
      <div style={{ ...t('mono'), fontSize: 18, color: RP.ink, marginTop: 4 }}>
        {v}{sub && <span style={{ fontSize: 11, color: RP.inkDim, marginLeft: 4 }}>{sub}</span>}
      </div>
    </div>
  );
}
