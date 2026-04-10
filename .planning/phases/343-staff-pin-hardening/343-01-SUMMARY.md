---
phase: 343-staff-pin-hardening
plan: 01
status: complete
commit: b31c38e0
---

# 343-01 Summary: Cloud-Authority 409 Guard on Venue Staff Mutations

## What shipped

Venue instance now returns HTTP 409 Conflict on PUT/POST/DELETE to staff endpoints when `staff_members` is in the cloud-authoritative tables list. Cloud instance accepts all mutations as before.

### Config changes (config.rs)
- Added `authoritative_tables: Vec<String>` to `CloudConfig` (default: `["staff_members"]`)
- Added `is_cloud_authoritative_for(table)` method on `CloudConfig`
- Added `this_instance_is_cloud(config)` helper (checks `RC_IS_CLOUD=1` env or self-referencing api_url)
- Added `allow_venue_staff_write()` helper (checks `RC_ALLOW_VENUE_STAFF_WRITE=1` env)

### Route changes (routes.rs)
- Added `cloud_authority_guard()` helper returning `Option<(409, Json)>` with error, cloud_url, hint, override_hint
- Wired into `create_staff`, `update_staff`, `delete_staff`, `reset_staff_pin` as first check

### 409 response format
```json
{
  "error": "staff_members is cloud-authoritative on this instance",
  "cloud_url": "http://100.70.177.44:8080/api/v1",
  "hint": "Submit your change to the cloud endpoint. It will sync to venue within 30s.",
  "override_hint": "Emergency: set RC_ALLOW_VENUE_STAFF_WRITE=1 on the venue instance and restart."
}
```

## Success criteria

- SC-1: Venue returns 409 on staff mutations ✅
- SC-2: Cloud accepts all staff mutations ✅
- Emergency override works ✅
- staff_validate_pin (read path) unaffected ✅

## Deferred

- Runtime verification on live venue + cloud (not deployed yet)
