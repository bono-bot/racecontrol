'use client';

// Racing Point V2 Kiosk — staff-operated, mouse+keyboard, 1280×800 landscape
// 13-step state machine: PIN → ForgotPIN → Register → PickGame → AcMode → AcSpSetup
//                       → AcMpLobby → Experience → Handoff → Live → Console → End → Results
// Source: claude.ai/design handoff bundle (PyiT2ipTf0VdYL8V9vd7-A) — 2026-05-02

import * as React from 'react';
import { RP } from '@/components/v2/tokens';
import type { KioskGame } from '@/components/v2/kiosk/KioskShared';

export type KioskStep =
  | 'pin'
  | 'forgotPin'
  | 'register'
  | 'pickGame'
  | 'acMode'
  | 'acSpSetup'
  | 'acMpLobby'
  | 'experience'
  | 'handoff'
  | 'live'
  | 'console'
  | 'end'
  | 'results';

export interface KioskFlowState {
  step: KioskStep;
  driverName?: string;
  driverClass?: 'Rookie' | 'Apex' | 'Podium' | 'Champion';
  game?: KioskGame;
  acMode?: 'sp' | 'mp';
}

const initialState: KioskFlowState = { step: 'pin', driverName: 'Uday Singh' };

// Lazy-load every screen so initial paint stays small (PIN gate only).
const KioskPin       = React.lazy(() => import('@/components/v2/kiosk/KioskPin').then(m => ({ default: m.KioskPin })));
const KioskForgotPin = React.lazy(() => import('@/components/v2/kiosk/KioskForgotPin').then(m => ({ default: m.KioskForgotPin })));
const KioskRegister  = React.lazy(() => import('@/components/v2/kiosk/KioskRegister').then(m => ({ default: m.KioskRegister })));
const KioskGamePick  = React.lazy(() => import('@/components/v2/kiosk/KioskGamePick').then(m => ({ default: m.KioskGamePick })));
const KioskAcMode    = React.lazy(() => import('@/components/v2/kiosk/KioskAc').then(m => ({ default: m.KioskAcMode })));
const KioskAcLobby   = React.lazy(() => import('@/components/v2/kiosk/KioskAc').then(m => ({ default: m.KioskAcLobby })));
const KioskAcSetup   = React.lazy(() => import('@/components/v2/kiosk/KioskAc').then(m => ({ default: m.KioskAcSetup })));
const KioskExperience= React.lazy(() => import('@/components/v2/kiosk/KioskExperience').then(m => ({ default: m.KioskExperience })));
const KioskHandoff   = React.lazy(() => import('@/components/v2/kiosk/KioskHandoff').then(m => ({ default: m.KioskHandoff })));
const KioskLive      = React.lazy(() => import('@/components/v2/kiosk/KioskLive').then(m => ({ default: m.KioskLive })));
const KioskConsole   = React.lazy(() => import('@/components/v2/kiosk/KioskConsole').then(m => ({ default: m.KioskConsole })));
const KioskResults   = React.lazy(() => import('@/components/v2/kiosk/KioskResults').then(m => ({ default: m.KioskResults })));

export default function KioskV2Page() {
  const [flow, setFlow] = React.useState<KioskFlowState>(initialState);

  const goTo = React.useCallback(
    (step: KioskStep, patch?: Partial<KioskFlowState>) =>
      setFlow(prev => ({ ...prev, ...patch, step })),
    [],
  );
  const reset = React.useCallback(() => setFlow(initialState), []);

  const fallback = <div style={{ color: RP.inkDim, padding: 40 }}>Loading…</div>;

  let body: React.ReactNode;
  switch (flow.step) {
    case 'pin':
      body = (
        <KioskPin
          mode="unlock"
          onSuccess={() => goTo('register')}
          onForgot={() => goTo('forgotPin')}
        />
      );
      break;
    case 'forgotPin':
      body = (
        <KioskForgotPin
          onBack={() => goTo('pin')}
          onSent={() => goTo('pin')}
        />
      );
      break;
    case 'register':
      body = (
        <KioskRegister
          onContinue={(driverName) => goTo('pickGame', { driverName })}
          onBack={reset}
        />
      );
      break;
    case 'pickGame':
      body = (
        <KioskGamePick
          driverName={flow.driverName}
          onPickAc={(game) => goTo('acMode', { game })}
          onPickSimple={(game) => goTo('experience', { game })}
          onBack={() => goTo('register')}
        />
      );
      break;
    case 'acMode':
      body = (
        <KioskAcMode
          onSinglePlayer={() => goTo('acSpSetup', { acMode: 'sp' })}
          onMultiplayer={() => goTo('acMpLobby', { acMode: 'mp' })}
          onBack={() => goTo('pickGame')}
        />
      );
      break;
    case 'acSpSetup':
      body = (
        <KioskAcSetup
          driverName={flow.driverName}
          onContinue={() => goTo('experience')}
          onBack={() => goTo('acMode')}
        />
      );
      break;
    case 'acMpLobby':
      body = (
        <KioskAcLobby
          onJoin={() => goTo('experience')}
          onBack={() => goTo('acMode')}
        />
      );
      break;
    case 'experience':
      body = (
        <KioskExperience
          driverName={flow.driverName}
          game={flow.game}
          mode={flow.acMode}
          onContinue={() => goTo('handoff')}
          onBack={() => goTo(flow.game?.branch === 'ac' ? (flow.acMode === 'mp' ? 'acMpLobby' : 'acSpSetup') : 'pickGame')}
        />
      );
      break;
    case 'handoff':
      body = (
        <KioskHandoff
          driverName={flow.driverName}
          game={flow.game}
          mode={flow.acMode}
          onContinue={() => goTo('live')}
        />
      );
      break;
    case 'live':
      body = (
        <KioskLive
          game={flow.game}
          driverName={flow.driverName}
          onSwitchGame={() => goTo('pickGame')}
          onEnd={() => goTo('end')}
        />
      );
      break;
    case 'console':
      body = (
        <KioskConsole
          mode="live"
          driverName={flow.driverName}
          game={flow.game}
          onPause={() => goTo('live')}
          onAbort={() => goTo('end')}
        />
      );
      break;
    case 'end':
      body = (
        <KioskConsole
          mode="finished"
          driverName={flow.driverName}
          game={flow.game}
          onEnd={() => goTo('results')}
        />
      );
      break;
    case 'results':
      body = <KioskResults onLock={reset} />;
      break;
  }

  return (
    <div style={{
      width: '100vw', height: '100vh', background: '#000',
      display: 'flex', alignItems: 'center', justifyContent: 'center', overflow: 'hidden',
    }}>
      <div style={{ width: 1280, height: 800, background: RP.asphalt, position: 'relative' }}>
        <React.Suspense fallback={fallback}>{body}</React.Suspense>
      </div>
    </div>
  );
}
