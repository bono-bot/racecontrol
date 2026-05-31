//! INC-1 contract test: the redeem-response parser mirrors the control-plane
//! route (`app/install/redeem/route.ts`) field-for-field, maps each documented
//! status code to a typed error, and the redeem URL stays a child of the single
//! baked tier-1 base.

use rc_installer::config::INSTALLER_TIER_1_BASE;
use rc_installer::identity::{
    default_redeem_url, interpret_response, parse_redeem_response, redeem_url, RedeemError,
};
use rc_installer::profile::Profile;

// A 200 body matching the route's shape (full mapping present). `r##"..."##`
// (two hashes) because the `racecontrol_toml` value begins with `"#`, which
// would terminate a single-hash raw string.
const FULL_BODY: &str = r##"{
  "tenant_id": "rp-hyd",
  "venue_id": "racingpoint-hyd-001",
  "profile": "pod",
  "server_address": "192.168.31.23",
  "bono_address": "srv1422716.hstgr.cloud",
  "racecontrol_toml": "# Provisioned by the RaceControl vendor console.\n[venue]\nname = \"Racing Point eSports\"\nvenue_id = \"racingpoint-hyd-001\"\n"
}"##;

#[test]
fn parses_a_full_redeem_body_field_for_field() {
    let id = parse_redeem_response(FULL_BODY).expect("full body parses");
    assert_eq!(id.tenant_id, "rp-hyd");
    assert_eq!(id.venue_id.as_deref(), Some("racingpoint-hyd-001"));
    assert_eq!(id.profile, Profile::Pod);
    assert_eq!(id.server_address.as_deref(), Some("192.168.31.23"));
    assert_eq!(id.bono_address.as_deref(), Some("srv1422716.hstgr.cloud"));
    assert!(id.racecontrol_toml.contains("[venue]"));
    assert_eq!(id.require_venue_id().unwrap(), "racingpoint-hyd-001");
}

#[test]
fn server_profile_label_maps_to_server() {
    let body = r#"{"tenant_id":"rp-hyd","venue_id":"racingpoint-hyd-001","profile":"server","racecontrol_toml":"x"}"#;
    let id = parse_redeem_response(body).expect("server body parses");
    assert_eq!(id.profile, Profile::Server);
}

#[test]
fn null_venue_id_becomes_none_and_require_is_a_hard_error() {
    let body = r#"{"tenant_id":"rp-hyd","venue_id":null,"profile":"pod","server_address":null,"bono_address":null,"racecontrol_toml":"x"}"#;
    let id = parse_redeem_response(body).expect("null-venue body parses");
    assert_eq!(id.venue_id, None);
    assert_eq!(id.server_address, None);
    // A missing venue_id is a HARD ERROR — never defaulted (handoff R3).
    assert_eq!(id.require_venue_id(), Err(RedeemError::VenueIdMissing));
}

#[test]
fn missing_required_fields_are_named() {
    let no_tenant = r#"{"venue_id":"v","profile":"pod","racecontrol_toml":"x"}"#;
    assert_eq!(
        parse_redeem_response(no_tenant),
        Err(RedeemError::MissingField("tenant_id"))
    );

    let no_profile = r#"{"tenant_id":"rp-hyd","racecontrol_toml":"x"}"#;
    assert_eq!(
        parse_redeem_response(no_profile),
        Err(RedeemError::MissingField("profile"))
    );

    let no_toml = r#"{"tenant_id":"rp-hyd","profile":"pod"}"#;
    assert_eq!(
        parse_redeem_response(no_toml),
        Err(RedeemError::MissingField("racecontrol_toml"))
    );
}

#[test]
fn unknown_profile_is_rejected_not_defaulted() {
    let body = r#"{"tenant_id":"rp-hyd","profile":"superuser","racecontrol_toml":"x"}"#;
    assert_eq!(
        parse_redeem_response(body),
        Err(RedeemError::UnknownProfile("superuser".to_string()))
    );
}

#[test]
fn syntactically_invalid_json_is_malformed() {
    match parse_redeem_response("{not json") {
        Err(RedeemError::Malformed(_)) => {}
        other => panic!("expected Malformed, got {other:?}"),
    }
}

#[test]
fn status_codes_map_to_typed_errors() {
    assert_eq!(RedeemError::from_status(404, ""), RedeemError::InvalidToken);
    assert_eq!(
        RedeemError::from_status(410, ""),
        RedeemError::ExpiredOrConsumed
    );
    assert_eq!(RedeemError::from_status(429, ""), RedeemError::RateLimited);
    match RedeemError::from_status(400, "{\"error\":\"missing_token\"}") {
        RedeemError::Malformed(d) => assert!(d.contains("missing_token")),
        other => panic!("expected Malformed, got {other:?}"),
    }
    assert_eq!(
        RedeemError::from_status(503, ""),
        RedeemError::UnexpectedStatus(503)
    );
}

#[test]
fn interpret_response_routes_status_to_parse_or_error() {
    // 2xx → parse the body.
    let ok = interpret_response(200, FULL_BODY).expect("2xx parses");
    assert_eq!(ok.tenant_id, "rp-hyd");
    // non-2xx → typed error.
    assert_eq!(interpret_response(404, ""), Err(RedeemError::InvalidToken));
    assert_eq!(
        interpret_response(410, ""),
        Err(RedeemError::ExpiredOrConsumed)
    );
    assert_eq!(interpret_response(429, ""), Err(RedeemError::RateLimited));
    // a 2xx with a broken body surfaces the PARSE error, not a status error.
    match interpret_response(200, "{bad") {
        Err(RedeemError::Malformed(_)) => {}
        other => panic!("expected Malformed, got {other:?}"),
    }
}

#[test]
fn redeem_url_is_a_child_of_the_tier1_base() {
    let url = default_redeem_url();
    assert!(
        url.starts_with(INSTALLER_TIER_1_BASE),
        "{url} must be under {INSTALLER_TIER_1_BASE}"
    );
    assert_eq!(url, format!("{INSTALLER_TIER_1_BASE}/install/redeem"));
    // A trailing slash on the base must not double up.
    assert_eq!(
        redeem_url("https://console.racecontrol.in/"),
        "https://console.racecontrol.in/install/redeem"
    );
}
