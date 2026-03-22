// Maps to Rust serde snake_case enums in crates/rc-common/src/types.rs

export type SimType =
  | "assetto_corsa"
  | "assetto_corsa_evo"
  | "assetto_corsa_rally"
  | "iracing"
  | "le_mans_ultimate"
  | "f1_25"
  | "forza"
  | "forza_horizon_5";

export type PodStatus = "offline" | "idle" | "in_session" | "error" | "disabled";

export type DrivingState = "active" | "idle" | "no_device";

export type GameState = "idle" | "launching" | "loading" | "running" | "stopping" | "error";

/** Maps to Rust PodInfo struct in crates/rc-common/src/types.rs */
export interface Pod {
  id: string;
  number: number;
  name: string;
  ip_address: string;
  mac_address?: string;
  sim_type: SimType;
  status: PodStatus;
  current_driver?: string;
  current_session_id?: string;
  last_seen?: string;
  driving_state?: DrivingState;
  billing_session_id?: string;
  game_state?: GameState;
  current_game?: SimType;
  installed_games?: SimType[];
  screen_blanked?: boolean;
  ffb_preset?: string;
  freedom_mode?: boolean;
}
