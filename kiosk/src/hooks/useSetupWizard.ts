"use client";

import { useState, useCallback } from "react";
import type {
  SetupStep,
  SessionType,
  ExperienceMode,
  AiDifficulty,
  CatalogItem,
  KioskExperience,
  Driver,
  PricingTier,
} from "@/lib/types";
import { DIFFICULTY_PRESETS } from "@/lib/constants";

export interface WizardState {
  currentStep: SetupStep;
  // Driver + Plan
  selectedDriver: Driver | null;
  selectedTier: PricingTier | null;
  // Game
  selectedGame: string;
  // Session type
  sessionType: SessionType;
  // AI config
  aiEnabled: boolean;
  aiDifficulty: AiDifficulty;
  aiCount: number;
  // Experience
  experienceMode: ExperienceMode;
  selectedExperience: KioskExperience | null;
  selectedTrack: CatalogItem | null;
  selectedCar: CatalogItem | null;
  // Driving settings
  drivingDifficulty: string;
  transmission: string;
  ffb: string;
  // Session splits removed (Act 2: one continuous timer)
}

const INITIAL_STATE: WizardState = {
  currentStep: "register_driver",
  selectedDriver: null,
  selectedTier: null,
  selectedGame: "",
  sessionType: "practice",
  aiEnabled: false,
  aiDifficulty: "easy",
  aiCount: 5,
  experienceMode: "preset",
  selectedExperience: null,
  selectedTrack: null,
  selectedCar: null,
  drivingDifficulty: "easy",
  transmission: "auto",
  ffb: "medium",
};

// Derive aids map from shared DIFFICULTY_PRESETS
const DIFFICULTY_AIDS: Record<string, Record<string, number>> = Object.fromEntries(
  Object.entries(DIFFICULTY_PRESETS).map(([k, v]) => [k, v.aids])
);

// Map kiosk AI difficulty names to numeric ai_level (0-100) for rc-agent's AcLaunchParams.
// Values are DifficultyTier midpoints from ac_launcher.rs.
const AI_DIFFICULTY_TO_LEVEL: Record<string, number> = {
  easy: 75,   // Rookie midpoint
  medium: 87, // Semi-Pro midpoint
  hard: 98,   // Alien midpoint
};

// Step flow (Act 2: no splits — one continuous timer)
const WIZARD_FLOW: SetupStep[] = [
  "register_driver",
  "select_plan",
  "select_game",
  "session_type",
  "ai_config",
  "select_experience",
  "select_track",
  "select_car",
  "driving_settings",
  "review",
];

export function useSetupWizard() {
  const [state, setState] = useState<WizardState>({ ...INITIAL_STATE });

  const setField = useCallback(<K extends keyof WizardState>(key: K, value: WizardState[K]) => {
    setState((prev) => ({ ...prev, [key]: value }));
  }, []);

  const goToStep = useCallback((step: SetupStep) => {
    setState((prev) => ({ ...prev, currentStep: step }));
  }, []);

  const getFlow = useCallback((): SetupStep[] => {
    const filtered = [...WIZARD_FLOW];
    const isAc = state.selectedGame === "assetto_corsa";

    // Non-AC games: skip all AC-specific steps (session_type, ai_config, track/car
    // selection, driving_settings). These games handle config internally — the kiosk
    // just picks a preset experience (duration) and launches via Steam.
    if (!isAc) {
      const acOnlySteps: SetupStep[] = [
        "session_type",
        "ai_config",
        "select_track",
        "select_car",
        "driving_settings",
        // Non-AC games use Steam-based launch with minimal args — skip experience
        // selection too (prevents dead-end when no experiences are configured for a game).
        "select_experience",
      ];
      // Flow: register_driver → select_plan → select_game → review
      return filtered.filter((s) => !acOnlySteps.includes(s));
    }

    // AC-specific flow below
    // If experience mode is "preset", skip select_track and select_car
    if (state.experienceMode === "preset") {
      return filtered.filter((s) => s !== "select_track" && s !== "select_car");
    }
    // If experience mode is "custom", skip select_experience
    return filtered.filter((s) => s !== "select_experience");
  }, [state.experienceMode, state.selectedGame, state.selectedTier]);

  const goBack = useCallback(() => {
    const flow = getFlow();
    const idx = flow.indexOf(state.currentStep);
    if (idx <= 0) return false; // Can't go back from first step
    setState((prev) => ({ ...prev, currentStep: flow[idx - 1] }));
    return true;
  }, [state.currentStep, getFlow]);

  const goNext = useCallback(() => {
    const flow = getFlow();
    const idx = flow.indexOf(state.currentStep);
    if (idx >= flow.length - 1) return false;
    setState((prev) => ({ ...prev, currentStep: flow[idx + 1] }));
    return true;
  }, [state.currentStep, getFlow]);

  const reset = useCallback(() => {
    setState({ ...INITIAL_STATE });
  }, []);

  const buildLaunchArgs = useCallback((): string => {
    const isAc = state.selectedGame === "assetto_corsa";

    // Non-AC games: minimal launch args — game handles config internally
    if (!isAc) {
      return JSON.stringify({
        game: state.selectedGame,
        driver: state.selectedDriver?.name || "",
        game_mode: "single",
      });
    }

    // AC: full launch args with track, car, assists, AI, etc.
    const aids = DIFFICULTY_AIDS[state.drivingDifficulty] || DIFFICULTY_AIDS.easy;

    const args: Record<string, unknown> = {
      car: state.selectedExperience?.car || state.selectedCar?.id || "",
      track: state.selectedExperience?.track || state.selectedTrack?.id || "",
      driver: state.selectedDriver?.name || "",
      difficulty: state.drivingDifficulty,
      transmission: state.transmission,
      ffb: state.ffb,
      game: state.selectedGame,
      game_mode: "single",
      aids,
      conditions: { damage: 0 },
      // session_type: rc-agent validates against VALID_SESSION_TYPES =
      // ["practice","hotlap","race","trackday","weekend","race_weekend"] (ac_launcher.rs:435).
      //
      // CRITICAL: KioskExperience.start_type is a DIFFERENT field — it stores AC's
      // pit/grid race-start position ("pitlane"|"grid") for the SESSION_0.START_RACE_FROM_PIT
      // INI key. The previous code conflated these two semantically distinct fields and
      // sent session_type="pitlane" for every preset experience, which rc-agent rejected
      // with "Unknown session_type". This stuck Pod 8 in maintenance_required state on
      // 2026-04-08 until the contract bug was found.
      //
      // All current seed experiences (db/mod.rs:921-926) are "Hot Lap" variants, so
      // preset experiences map to session_type="hotlap". Future enhancement: add a
      // session_type column to kiosk_experiences for per-preset override.
      session_type: state.selectedExperience ? "hotlap" : state.sessionType,
      // ai_level: numeric 0-100 value matching rc-agent's AcLaunchParams.ai_level
      ai_level: AI_DIFFICULTY_TO_LEVEL[state.aiDifficulty] ?? 87,
      // ai_count: how many AI opponents to generate (agent auto-picks car models)
      ai_count: state.aiEnabled ? state.aiCount : 0,
    };

    // Race Weekend: allocate sub-session times (practice 20%, qualify 20%, race gets remainder).
    // duration_minutes is injected server-side from billing, but these proportions are fixed.
    // The agent uses weekend_practice_minutes/weekend_qualify_minutes from launch_args.
    // Minimum 5 min per sub-session: AC requires out-lap before timed laps, so <5min = 0 viable laps.
    if (state.sessionType === "race_weekend") {
      const tierDuration = state.selectedTier?.duration_minutes ?? 30;
      const MIN_SUB_SESSION = 5; // AC out-lap + 1 flying lap needs ~4-5 min on most tracks
      const practice = Math.max(MIN_SUB_SESSION, Math.floor(tierDuration * 0.2));
      const qualify = Math.max(MIN_SUB_SESSION, Math.floor(tierDuration * 0.2));
      // Guard: if practice+qualify would exceed 80% of total, cap both to fit
      const maxSubTotal = Math.floor(tierDuration * 0.8);
      if (practice + qualify > maxSubTotal) {
        const half = Math.floor(maxSubTotal / 2);
        args.weekend_practice_minutes = Math.max(1, half);
        args.weekend_qualify_minutes = Math.max(1, half);
      } else {
        args.weekend_practice_minutes = practice;
        args.weekend_qualify_minutes = qualify;
      }
    }

    return JSON.stringify(args);
  }, [state]);

  const isFirstStep = state.currentStep === getFlow()[0];

  return {
    state,
    setField,
    goToStep,
    goBack,
    goNext,
    reset,
    buildLaunchArgs,
    isFirstStep,
  };
}
