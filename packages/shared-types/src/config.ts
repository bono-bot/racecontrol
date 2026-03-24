/** v22.0 Phase 177: Feature flag from the server registry */
export interface FeatureFlag {
  name: string;
  enabled: boolean;
  default_value: boolean;
  overrides: Record<string, boolean>;
  version: number;
  updated_at: string;
}

/** v22.0 Phase 177: Config push queue entry */
export interface ConfigPush {
  id: number;
  pod_id: string;
  payload: Record<string, unknown>;
  seq_num: number;
  status: 'pending' | 'delivered' | 'acked';
  created_at: string;
  acked_at?: string;
}

/** v22.0 Phase 177: Config audit log entry */
export interface ConfigAuditEntry {
  id: number;
  action: string;
  entity_type: string;
  entity_name: string;
  old_value?: string;
  new_value?: string;
  pushed_by: string;
  pods_acked: string[];
  created_at: string;
}
