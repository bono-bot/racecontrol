"use client";

import { useState, useCallback } from "react";
import type {
  SetupStep,
  SessionType,
  PlayerMode,
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
  // Player mode + Session type
  playerMode: PlayerMode;
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
  // Session splits (AC only)
  splitCount: number;
  splitDurationMinutes: number | null;
  // Multiplayer
  multiplayerMode: "create" | "join" | null;
  serverIp: string;
  serverPort: string;
  serverHttpPort: string;
  serverPassword: string;
}

const INITIAL_STATE: WizardState = {
  currentStep: "register_driver",
  selectedDriver: null,
  selectedTier: null,
  selectedGame: "",
  playerMode: "single",
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
  splitCount: 1,
  splitDurationMinutes: null,
  multiplayerMode: null,
  serverIp: "",
  serverPort: "",
  serverHttpPort: "",
  serverPassword: "",
};

// Derive aids map from shared DIFFICULTY_PRESETS
const DIFFICULTY_AIDS: Record<string, Record<string, number>> = Object.fromEntries(
  Object.entries(DIFFICULTY_PRESETS).map(([k, v]) => [k, v.aids])
);

// Step flow for single player
const SINGLE_FLOW: SetupStep[] = [
  "register_driver",
  "select_plan",
  "select_game",
  "session_splits",
  "player_mode",
  "session_type",
  "ai_config",
  "select_experience",
  "select_track",
  "select_car",
  "driving_settings",
  "review",
];

// Step flow for multiplayer
const MULTI_FLOW: SetupStep[] = [
  "register_driver",
  "select_plan",
  "select_game",
  "player_mode",
  "multiplayer_lobby",
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
    const flow = state.playerMode === "multi" ? [...MULTI_FLOW] : [...SINGLE_FLOW];
    let filtered = flow;
    // Skip session_splits for non-AC games or if tier duration < 20 min (no valid splits)
    const isAc = state.selectedGame === "assetto_corsa";
    const duration = state.selectedTier?.duration_minutes ?? 0;
    if (!isAc || duration < 20) {
      filtered = filtered.filter((s) => s !== "session_splits");
    }
    // If experience mode is "preset", skip select_track and select_car
    if (state.experienceMode === "preset") {
      return filtered.filter((s) => s !== "select_track" && s !== "select_car");
    }
    // If experience mode is "custom", skip select_experience
    return filtered.filter((s) => s !== "select_experience");
  }, [state.playerMode, state.experienceMode, state.selectedGame, state.selectedTier]);

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
    const aids = DIFFICULTY_AIDS[state.drivingDifficulty] || DIFFICULTY_AIDS.easy;
    const isMulti = state.playerMode === "multi";

    const args: Record<string, unknown> = {
      car: state.selectedExperience?.car || state.selectedCar?.id || "",
      track: state.selectedExperience?.track || state.selectedTrack?.id || "",
      driver: state.selectedDriver?.name || "",
      difficulty: state.drivingDifficulty,
      transmission: state.transmission,
      ffb: state.ffb,
      game: state.selectedGame,
      game_mode: isMulti ? "multi" : "single",
      aids,
      conditions: { damage: 0 },
      // New fields
      session_type: state.sessionType,
      ai_enabled: state.aiEnabled,
      ai_difficulty: state.aiDifficulty,
      ai_count: state.aiEnabled ? state.aiCount : 0,
    };

    // Multiplayer fields
    if (isMulti) {
      args.server_ip = state.serverIp;
      args.server_port = state.serverPort;
      args.server_http_port = state.serverHttpPort;
      args.server_password = state.serverPassword;
      args.multiplayer_mode = state.multiplayerMode;
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
