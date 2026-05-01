'use client';

// Racing Point V2 Kiosk — staff-operated, mouse+keyboard, 1280×800 landscape
// 13-step state machine: PIN → ForgotPIN → Register → PickGame → AcMode → AcSpSetup
//                       → AcMpLobby → Experience → Handoff → Live → Console → End → Results
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP, FONT, t } from '@/components/v2/tokens';

// Step indices match design's flow numbering
export type KioskStep =
  | 'pin'         // 0
  | 'forgotPin'   // 1
  | 'register'    // 2  driver name + autocomplete
  | 'pickGame'    // 3  8 games (AC = full-setup branch; others = simple)
  | 'acMode'      // 4  AC SP/MP
  | 'acSpSetup'   // 5  AC SP transmission + aids + AI toggle
  | 'acMpLobby'   // 6  AC MP lobby + AI
  | 'experience'  // 7  session picker (parameterized by game)
  | 'handoff'     // 8  pod-handed-off countdown
  | 'live'        // 9  live session (sim-cockpit HUD for AC)
  | 'console'     // 10 operator console (Pause / Extend / Switch Game / Blank / Abort)
  | 'end'         // 11 confirm end session
  | 'results';    // 12 podium + class progress + QR-to-PWA

export interface KioskFlowState {
  step: KioskStep;
  driverName?: string;
  driverClass?: 'Rookie' | 'Apex' | 'Podium' | 'Champion';
  game?: string;        // 'ac' | 'iracing' | etc.
  acMode?: 'sp' | 'mp';
  transmission?: 'manual' | 'auto';
  aidLevel?: 'easy' | 'hard' | 'ultra' | 'extreme';
  aiEnabled?: boolean;
  aiDifficulty?: 'easy' | 'medium' | 'hard' | 'pro';
  experience?: string;  // session preset id
}

const initialState: KioskFlowState = { step: 'pin' };

// Placeholder for screens not yet implemented in this batch.
function PlaceholderScreen({ step, onBack, onNext }: { step: KioskStep; onBack?: () => void; onNext?: () => void }) {
  return (
    <div style={{
      width: '100%', height: '100%', background: RP.asphalt, color: RP.ink,
      fontFamily: FONT.body, display: 'flex', alignItems: 'center', justifyContent: 'center',
      flexDirection: 'column', gap: 24,
    }}>
      <div style={{ ...t('caption'), color: RP.amber }}>● V2 KIOSK · STEP NOT YET PORTED</div>
      <div style={{ ...t('h1'), fontFamily: FONT.display, fontSize: 36, color: RP.ink }}>
        {step.toUpperCase()}
      </div>
      <div style={{ ...t('body'), color: RP.inkDim, maxWidth: 480, textAlign: 'center' }}>
        This screen is part of the 13-step kiosk flow but hasn't been ported from the
        design bundle yet. The PIN gate is the only working screen at this checkpoint.
      </div>
      <div style={{ display: 'flex', gap: 12, marginTop: 16 }}>
        {onBack && (
          <button onClick={onBack} style={{
            ...t('caption'), padding: '12px 20px', background: 'transparent',
            color: RP.inkDim, border: `1px solid ${RP.border}`, cursor: 'pointer',
          }}>← Back</button>
        )}
        {onNext && (
          <button onClick={onNext} style={{
            ...t('caption'), padding: '12px 20px', background: RP.red, color: '#fff',
            border: 'none', cursor: 'pointer',
          }}>Skip → next</button>
        )}
      </div>
    </div>
  );
}

export default function KioskV2Page() {
  const [flow, setFlow] = React.useState<KioskFlowState>(initialState);

  const goTo = React.useCallback((step: KioskStep, patch?: Partial<KioskFlowState>) => {
    setFlow(prev => ({ ...prev, ...patch, step }));
  }, []);

  const reset = React.useCallback(() => setFlow(initialState), []);

  // Lazy import the PIN screen so this entry point stays small until other
  // screens are added. Once all screens are ported this can be a switch().
  const KioskPin = React.useMemo(
    () => React.lazy(() => import('@/components/v2/kiosk/KioskPin').then(m => ({ default: m.KioskPin }))),
    []
  );

  let body: React.ReactNode;
  switch (flow.step) {
    case 'pin':
      body = (
        <React.Suspense fallback={<div style={{ color: RP.inkDim, padding: 40 }}>Loading…</div>}>
          <KioskPin
            mode="unlock"
            onSuccess={() => goTo('register')}
            onForgot={() => goTo('forgotPin')}
          />
        </React.Suspense>
      );
      break;
    case 'forgotPin':
      body = <PlaceholderScreen step="forgotPin" onBack={() => goTo('pin')} onNext={() => goTo('pin')} />;
      break;
    case 'register':
      body = <PlaceholderScreen step="register" onBack={reset} onNext={() => goTo('pickGame')} />;
      break;
    case 'pickGame':
      body = <PlaceholderScreen step="pickGame" onBack={() => goTo('register')} onNext={() => goTo('acMode', { game: 'ac' })} />;
      break;
    case 'acMode':
      body = <PlaceholderScreen step="acMode" onBack={() => goTo('pickGame')} onNext={() => goTo('acSpSetup', { acMode: 'sp' })} />;
      break;
    case 'acSpSetup':
      body = <PlaceholderScreen step="acSpSetup" onBack={() => goTo('acMode')} onNext={() => goTo('experience')} />;
      break;
    case 'acMpLobby':
      body = <PlaceholderScreen step="acMpLobby" onBack={() => goTo('acMode')} onNext={() => goTo('experience')} />;
      break;
    case 'experience':
      body = <PlaceholderScreen step="experience" onBack={() => goTo('pickGame')} onNext={() => goTo('handoff')} />;
      break;
    case 'handoff':
      body = <PlaceholderScreen step="handoff" onBack={() => goTo('experience')} onNext={() => goTo('live')} />;
      break;
    case 'live':
      body = <PlaceholderScreen step="live" onNext={() => goTo('console')} />;
      break;
    case 'console':
      body = <PlaceholderScreen step="console" onBack={() => goTo('live')} onNext={() => goTo('end')} />;
      break;
    case 'end':
      body = <PlaceholderScreen step="end" onBack={() => goTo('console')} onNext={() => goTo('results')} />;
      break;
    case 'results':
      body = <PlaceholderScreen step="results" onNext={reset} />;
      break;
  }

  // Page wrapper sized to kiosk canvas (1280×800 landscape per design).
  return (
    <div style={{
      width: '100vw', height: '100vh', background: '#000',
      display: 'flex', alignItems: 'center', justifyContent: 'center', overflow: 'hidden',
    }}>
      <div style={{ width: 1280, height: 800, background: RP.asphalt, position: 'relative' }}>
        {body}
      </div>
    </div>
  );
}
