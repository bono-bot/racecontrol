/** v22.0: Server -> Agent flag sync payload (CoreToAgentMessage::FlagSync) */
export interface FlagSyncPayload {
  flags: Record<string, boolean>;
  version: number;
}

/** v22.0: Server -> Agent config push payload (CoreToAgentMessage::ConfigPush) */
export interface WsConfigPushPayload {
  fields: Record<string, unknown>;
  schema_version: number;
  sequence: number;
}

/** v22.0: Server -> Agent OTA download command (CoreToAgentMessage::OtaDownload) */
export interface OtaDownloadPayload {
  manifest_url: string;
  binary_sha256: string;
  version: string;
}

/** v22.0: Server -> Agent kill switch (CoreToAgentMessage::KillSwitch) */
export interface KillSwitchPayload {
  flag_name: string;
  active: boolean;
  reason?: string;
}

/** v22.0: Agent -> Server config ack (AgentMessage::ConfigAck) */
export interface ConfigAckPayload {
  pod_id: string;
  sequence: number;
  accepted: boolean;
}

/** v22.0: Agent -> Server OTA ack (AgentMessage::OtaAck) */
export interface OtaAckPayload {
  pod_id: string;
  version: string;
  success: boolean;
  error?: string;
}

/** v22.0: Agent -> Server flag cache sync request (AgentMessage::FlagCacheSync) */
export interface FlagCacheSyncPayload {
  pod_id: string;
  cached_version: number;
}
