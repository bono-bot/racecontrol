#[cfg(test)]
mod config_push_tests {
    use super::super::config_push_full::compute_config_hash;
    use rc_common::config_schema::AgentConfig;
    use rc_common::types::FullConfigPushPayload;
    use rc_common::protocol::CoreToAgentMessage;

    /// Test 1: FullConfigPushPayload serializes to JSON with config, config_hash, schema_version fields
    #[test]
    fn full_config_push_payload_serializes_expected_fields() {
        let config = AgentConfig::default();
        let config_hash = compute_config_hash(&config);
        let payload = FullConfigPushPayload {
            schema_version: config.schema_version,
            config,
            config_hash: config_hash.clone(),
        };
        let json = serde_json::to_string(&payload).expect("should serialize");
        assert!(json.contains("\"config\""), "JSON must have 'config' field: {json}");
        assert!(json.contains("\"config_hash\""), "JSON must have 'config_hash' field: {json}");
        assert!(json.contains("\"schema_version\""), "JSON must have 'schema_version' field: {json}");
        assert!(json.contains(&config_hash), "JSON must contain the hash: {json}");
    }

    /// Test 2: FullConfigPushPayload round-trips through serde
    #[test]
    fn full_config_push_payload_serde_roundtrip() {
        let config = AgentConfig::default();
        let config_hash = compute_config_hash(&config);
        let payload = FullConfigPushPayload {
            schema_version: config.schema_version,
            config,
            config_hash: config_hash.clone(),
        };
        let json = serde_json::to_string(&payload).expect("should serialize");
        let decoded: FullConfigPushPayload = serde_json::from_str(&json).expect("should deserialize");
        assert_eq!(decoded.config_hash, config_hash);
        assert_eq!(decoded.schema_version, 1);
    }

    /// Test 3: CoreToAgentMessage::FullConfigPush variant serializes with type="full_config_push"
    #[test]
    fn core_to_agent_full_config_push_has_correct_type_tag() {
        let config = AgentConfig::default();
        let config_hash = compute_config_hash(&config);
        let payload = FullConfigPushPayload {
            schema_version: config.schema_version,
            config,
            config_hash,
        };
        let msg = CoreToAgentMessage::FullConfigPush(payload);
        let json = serde_json::to_string(&msg).expect("should serialize");
        assert!(json.contains("\"full_config_push\""), "type tag must be full_config_push: {json}");
    }

    /// Test 4: compute_config_hash returns consistent SHA-256 hex for same AgentConfig
    #[test]
    fn compute_config_hash_is_consistent() {
        let config = AgentConfig::default();
        let hash1 = compute_config_hash(&config);
        let hash2 = compute_config_hash(&config);
        assert_eq!(hash1, hash2, "Same config must produce same hash");
        // SHA-256 hex = 64 chars
        assert_eq!(hash1.len(), 64, "SHA-256 hex should be 64 chars");
    }

    /// Test 5: compute_config_hash returns different hash for different AgentConfig values
    #[test]
    fn compute_config_hash_differs_for_different_configs() {
        let mut config1 = AgentConfig::default();
        let mut config2 = AgentConfig::default();
        config1.pod.number = 1;
        config2.pod.number = 2;
        let hash1 = compute_config_hash(&config1);
        let hash2 = compute_config_hash(&config2);
        assert_ne!(hash1, hash2, "Different configs must produce different hashes");
    }
}
