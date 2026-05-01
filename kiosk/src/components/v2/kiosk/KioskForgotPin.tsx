'use client';

// Racing Point V2 Kiosk — Forgot PIN (2-stage WhatsApp recovery)
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '../tokens';
import { Icon, PrimaryCTA, SecondaryCTA } from '../atomic';
import { KioskFrame } from './KioskFrame';

export function KioskForgotPin({ onBack, onSent }: {
  onBack: () => void;
  onSent: () => void;
}) {
  const [stage, setStage] = React.useState<'confirm' | 'sent'>('confirm');

  return (
    <KioskFrame podId="POD-01" state="idle" hideStatus>
      <div style={{ height: '100%', display: 'flex', alignItems: 'center', justifyContent: 'center', padding: 40 }}>
        <div style={{ width: 520, background: RP.card, border: `1px solid ${RP.border}`, padding: 32 }}>
          {stage === 'confirm' ? (
            <>
              <div style={{ ...t('caption'), fontSize: 10, color: RP.amber }}>● PIN RECOVERY</div>
              <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 26, color: RP.ink, marginTop: 8 }}>
                Send new PIN via WhatsApp?
              </div>
              <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 12, lineHeight: 1.6 }}>
                We&apos;ll generate a new 4-digit PIN and send it to the operator&apos;s registered WhatsApp number.
                The previous PIN will stop working immediately.
              </div>

              <div style={{ marginTop: 24, padding: '14px 18px', background: RP.base, border: `1px solid ${RP.border}` }}>
                <div style={{ ...t('caption'), fontSize: 9, color: RP.gunmetal }}>SENDING TO</div>
                <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginTop: 4 }}>
                  <div style={{ width: 24, height: 24, background: '#25D366', borderRadius: 4, display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                    <Icon name="phone" size={14} color="#fff" />
                  </div>
                  <div style={{ ...t('mono'), fontSize: 14, color: RP.ink }}>+91 ••••• •••42</div>
                  <div style={{ ...t('caption'), fontSize: 9, color: RP.green, marginLeft: 'auto' }}>VERIFIED</div>
                </div>
              </div>

              <div style={{ display: 'flex', gap: 10, marginTop: 24 }}>
                <SecondaryCTA onClick={onBack} shortcut="ESC">Cancel</SecondaryCTA>
                <div style={{ flex: 1 }} />
                <PrimaryCTA onClick={() => setStage('sent')} shortcut="ENTER">Send New PIN</PrimaryCTA>
              </div>
              <div style={{ ...t('bodySm'), fontSize: 11, color: RP.gunmetal, marginTop: 16, textAlign: 'center' }}>
                Wrong number? Ask the manager to update it in Admin → Settings → Staff.
              </div>
            </>
          ) : (
            <>
              <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
                <div style={{ width: 32, height: 32, borderRadius: '50%', background: RP.green, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  <Icon name="check" size={18} color={RP.asphalt} />
                </div>
                <div style={{ ...t('caption'), fontSize: 10, color: RP.green }}>● SENT</div>
              </div>
              <div style={{ ...t('h2'), fontFamily: FONT.display, fontSize: 26, color: RP.ink, marginTop: 16 }}>
                New PIN sent
              </div>
              <div style={{ ...t('body'), fontSize: 13, color: RP.inkDim, marginTop: 12, lineHeight: 1.6 }}>
                Check WhatsApp on +91 ••••• •••42. The new 4-digit PIN should arrive within 30 seconds.
                The previous PIN is no longer valid.
              </div>
              <div style={{ display: 'flex', justifyContent: 'flex-end', marginTop: 24 }}>
                <PrimaryCTA onClick={onSent} shortcut="ENTER">Back to PIN entry</PrimaryCTA>
              </div>
            </>
          )}
        </div>
      </div>
    </KioskFrame>
  );
}
