# Racing Point RaceControl — API Reference

**Base URL:** `http://192.168.31.23:8080/api/v1/`
**Framework:** Rust/Axum
**Auth:** JWT (staff/customer), rate limiting, RBAC
**Total Endpoints:** ~403 HTTP routes across 7 tiers
**Route Definition:** `crates/racecontrol/src/api/routes.rs`

---

## Router Architecture

```rust
pub fn api_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .merge(auth_rate_limited_routes())       // 5 req/min per IP
        .merge(public_routes())                  // No auth
        .merge(customer_routes())                // Customer JWT (in-handler)
        .merge(kiosk_routes(state))              // Staff JWT + pod-accessible
        .merge(staff_routes(state))              // Staff JWT + pod-source-block
        .merge(service_routes())                 // Service auth (terminal, sync, bot)
        .merge(survival::survival_routes())      // Lease/heartbeat
        .merge(fleet_healer::fleet_healer_routes())
}
```

| Tier | Auth | ~Count | Purpose |
|------|------|--------|---------|
| Public | None | 65 | Health, venue, leaderboards, registration |
| Auth (Rate-Limited) | Rate limit only | 8 | Login, PIN, OTP endpoints |
| Customer | JWT (in-handler) | 75 | Profile, booking, wallet, friends, multiplayer |
| Kiosk | Staff JWT + pod OK | 5 | Kiosk experiences, settings, launch |
| Staff/Admin | Staff JWT + pod block | 200 | Pod mgmt, billing, games, reports, deploy |
| Service | terminal_secret | 35 | Cloud sync, terminal, bot |

---

## Tier 1: Public Endpoints (No Auth)

### Health & Status

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/health` | `health()` | Server health (build_id, uptime) |
| GET | `/fleet/health` | `fleet_health_handler()` | All 8 pods status (ws_connected, http_reachable, build_id) |
| GET | `/pod-status-summary` | `pod_status_summary()` | Summary of pod states |
| GET | `/app-health` | — | Dashboard app health check |
| GET | `/backup/status` | `get_backup_status()` | Backup system status |
| GET | `/deploy-log` | — | Deploy event history |
| GET | `/metrics/prometheus` | `prometheus_handler()` | Prometheus exposition format |

### Pod Availability

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/pods/{id}/availability` | `pod_availability_handler()` | Check if pod is available |

### Crash & Error Reporting

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/sentry/crash` | `sentry_crash_handler()` | rc-sentry reports rc-agent crash |
| POST | `/fleet/blocked-start` | `blocked_start_handler()` | Pod reports game launch blocked |
| POST | `/telemetry/client-error` | `client_error_handler()` | Browser/kiosk error logging |
| POST | `/recovery/events` | `post_recovery_event()` | Recovery event reporting |
| GET | `/recovery/events` | `get_recovery_events()` | Recovery event list |

### Venue & Registration

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/venue` | `venue_info()` | Venue name, location, hours |
| POST | `/venue/register` | `venue_register()` | Register new venue |
| POST | `/customer/register` | `customer_register()` | Public customer registration (PWA) |

### Leaderboards & Records

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/public/leaderboard` | `public_leaderboard()` | Global leaderboard |
| GET | `/public/leaderboard/{track}` | `public_track_leaderboard()` | Per-track leaderboard |
| GET | `/public/circuit-records` | `public_circuit_records()` | Best laps per track |
| GET | `/public/vehicle-records/{car}` | `public_vehicle_records()` | Per-vehicle records |
| GET | `/public/drivers` | `public_drivers_search()` | Driver search/list |
| GET | `/public/drivers/{id}` | `public_driver_profile()` | Driver profile + stats |
| GET | `/public/drivers/{id}/rating` | `public_driver_rating()` | Driver skill rating |
| GET | `/public/time-trial` | `public_time_trial()` | Time-trial standings |
| GET | `/public/laps/{lap_id}/telemetry` | `public_lap_telemetry()` | Lap telemetry replay |
| GET | `/public/sessions/{id}` | `public_session_summary()` | Session results |
| GET | `/public/championships` | `public_championships_list()` | Championship list |
| GET | `/public/championships/{id}` | `public_championship_standings()` | Championship standings |
| GET | `/public/championships/{id}/standings` | `public_championship_standings_handler()` | Same |
| GET | `/public/events` | `public_events_list()` | Hotlap events |
| GET | `/public/events/{id}` | `public_event_leaderboard()` | Event leaderboard |
| GET | `/public/events/{id}/sessions` | `public_event_sessions()` | Event sessions |

### Wallet & Pricing (Public)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/wallet/bonus-tiers` | `wallet_bonus_tiers()` | Bonus structures |
| GET | `/pricing/display` | `pricing_display_handler()` | Public pricing (2x psychology variants) |
| GET | `/pricing/social-proof` | `pricing_social_proof_handler()` | "N others playing now" |

### Config (Read-Only)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/config/kiosk-allowlist` | `list_kiosk_allowlist()` | Process guard allowlist (rc-agent fetches) |
| GET | `/guard/whitelist/{machine_id}` | `get_whitelist_handler()` | Per-pod process whitelist |
| GET | `/presets` | `list_presets()` | Game presets (controls, camera, difficulty) |
| GET | `/presets/{id}` | `get_preset()` | Preset details |
| GET | `/fleet/pod-inventory/{pod_id}` | `pod_inventory_handler()` | Games installed on pod |

### Cafe (Public)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/cafe/menu` | `public_menu()` | Customer-facing menu |
| GET | `/cafe/promos/active` | `list_active_promos()` | Current promotions |

### Queue

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/queue/join` | `queue_join_handler()` | Join wait queue |
| GET | `/queue/status/{id}` | `queue_status_handler()` | Check position |
| POST | `/queue/{id}/leave` | `queue_leave_handler()` | Leave queue |

### Misc Public

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/cameras/health` | `cameras_health_proxy()` | go2rtc health proxy |
| POST | `/fleet/alert` | `post_fleet_alert()` | Escalate to WhatsApp |
| GET | `/legal/minor-waiver-disclosure` | `minor_waiver_disclosure()` | Minor waiver text |
| GET/POST | `/kiosk/ping` | `kiosk_ping_handler()` | Kiosk heartbeat |
| POST | `/billing/{id}/agent-shutdown` | `agent_shutdown_handler()` | rc-agent graceful shutdown |
| GET | `/billing/pod/{pod_id}/interrupted` | `interrupted_sessions_handler()` | Check interrupted billing |
| POST | `/webhooks/payment-gateway` | `payment_gateway_webhook()` | Payment webhook |
| GET | `/customer/otp-fallback/{token}` | `otp_fallback_handler()` | OTP display token |
| GET | `/pos/lockdown` | `get_pos_lockdown()` | POS lock status |

---

## Tier 2: Auth Endpoints (Rate-Limited, 5 req/min)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/customer/login` | `customer_login()` | Phone + OTP flow start |
| POST | `/customer/resend-otp` | `customer_resend_otp()` | Resend OTP |
| POST | `/customer/verify-otp` | `customer_verify_otp()` | Verify OTP -> customer JWT |
| POST | `/auth/validate-pin` | `validate_pin()` | Customer PIN -> JWT |
| POST | `/auth/kiosk/validate-pin` | `kiosk_validate_pin()` | Kiosk display PIN -> guest session |
| POST | `/kiosk/redeem-pin` | `kiosk_redeem_pin()` | Promo PIN redemption |
| POST | `/staff/validate-pin` | `staff_validate_pin()` | Staff PIN -> staff JWT |
| POST | `/auth/admin-login` | `admin_login()` | Admin password -> superadmin JWT |

---

## Tier 3: Customer Endpoints (JWT in-handler)

### Profile & Status

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/profile` | `customer_profile()` | Profile (name, email, phone, DOB) |
| PUT | `/customer/profile` | `customer_update_profile()` | Update profile |
| GET | `/customer/waiver-status` | `customer_waiver_status()` | Waiver signed/pending |
| GET | `/customer/racers` | `customer_list_racers()` | Racers under account |
| POST | `/customer/racers` | `customer_add_racer()` | Add racer |
| GET | `/customer/stats` | `customer_stats()` | Aggregate stats |

### Sessions & Telemetry

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/sessions` | `customer_sessions()` | Past sessions |
| GET | `/customer/sessions/{id}` | `customer_session_detail()` | Session detail |
| GET | `/customer/sessions/{id}/share` | `customer_session_share()` | Shareable report link |
| GET | `/customer/sessions/{id}/receipt` | `customer_session_receipt()` | Financial receipt |
| GET | `/customer/sessions/{id}/invoice` | `customer_session_invoice()` | GST invoice |
| GET | `/customer/laps` | `customer_laps()` | All laps |
| GET | `/customer/telemetry` | `customer_telemetry()` | Speed, throttle, braking data |
| GET | `/customer/compare-laps` | `customer_compare_laps()` | Lap comparison (coaching) |

### Booking & Reservation

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/experiences` | `customer_experiences()` | Package offerings |
| GET | `/customer/ac/catalog` | `customer_ac_catalog()` | AC cars & tracks |
| POST | `/customer/book` | `customer_book_session()` | Immediate booking |
| GET | `/customer/active-reservation` | `customer_active_reservation()` | Current session |
| POST | `/customer/end-reservation` | `customer_end_reservation()` | End early |
| POST | `/customer/continue-session` | `customer_continue_session()` | Continue after pause |
| GET | `/customer/reservation` | `customer_get_reservation()` | Pending reservation |
| POST | `/customer/reservation/create` | `customer_create_reservation()` | Future booking |
| PUT | `/customer/reservation/modify` | `customer_modify_reservation()` | Reschedule |
| DELETE | `/customer/reservation` | `customer_cancel_reservation()` | Cancel |
| POST | `/customer/game-request` | `pwa_game_request()` | Request game launch (staff confirms) |
| GET | `/customer/game-request/{id}` | `get_game_request_status()` | Poll for approval |

### Wallet

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/wallet` | `customer_wallet()` | Balance, bonus, expiry |
| GET | `/customer/wallet/transactions` | `customer_wallet_transactions()` | Transaction history |
| POST | `/customer/apply-coupon` | `customer_apply_coupon()` | Apply coupon |

### Social & Multiplayer

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/friends` | `customer_friends()` | Friend list |
| GET | `/customer/friends/requests` | `customer_friend_requests()` | Pending requests |
| POST | `/customer/friends/request` | `customer_send_friend_request()` | Send request |
| POST | `/customer/friends/request/{id}/accept` | `customer_accept_friend_request()` | Accept |
| POST | `/customer/friends/request/{id}/reject` | `customer_reject_friend_request()` | Reject |
| DELETE | `/customer/friends/{id}` | `customer_remove_friend()` | Unfriend |
| PUT | `/customer/presence` | `customer_set_presence()` | Online/away/busy |
| POST | `/customer/book-multiplayer` | `customer_book_multiplayer()` | Multiplayer request |
| GET | `/customer/group-session` | `customer_group_session()` | Group session |
| POST | `/customer/group-session/{id}/accept` | `customer_accept_group_invite()` | Accept invite |
| POST | `/customer/group-session/{id}/decline` | `customer_decline_group_invite()` | Decline |
| GET | `/customer/multiplayer-results/{id}` | `customer_multiplayer_results()` | Results |

### Gamification & Passport

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/passport` | `customer_passport()` | Driving passport/license |
| GET | `/customer/badges` | `customer_badges()` | Achievement badges |
| GET | `/customer/active-session/events` | `customer_active_session_events()` | Live PB/milestone events |
| GET | `/customer/referral-code` | `customer_referral_code()` | Referral code |
| POST | `/customer/referral-code/generate` | `customer_generate_referral_code()` | Generate code |
| POST | `/customer/redeem-referral` | `customer_redeem_referral()` | Redeem referral |

### Tournaments & Memberships

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/tournaments` | `customer_list_tournaments()` | Active tournaments |
| POST | `/customer/tournaments/{id}/register` | `customer_register_tournament()` | Register |
| GET | `/customer/packages` | `customer_list_packages()` | Pre-paid packages |
| GET | `/customer/membership` | `customer_membership()` | Membership status |
| POST | `/customer/membership/subscribe` | `customer_subscribe_membership()` | Subscribe |

### Data Rights (DPDP Act)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/customer/data-export` | `customer_data_export()` | Download all personal data |
| DELETE | `/customer/data-delete` | `customer_data_delete()` | Request deletion |
| POST | `/customer/revoke-consent` | `revoke_consent_handler()` | Revoke DPDP consent |
| POST | `/customer/dispute` | `create_dispute_handler()` | File billing dispute |

### Cafe & AI

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/customer/cafe/orders` | `place_cafe_order_customer()` | Place order |
| GET | `/customer/cafe/orders/history` | `list_customer_orders()` | Order history |
| POST | `/customer/ai/chat` | `customer_ai_chat()` | AI coach chat |

---

## Tier 4: Kiosk Endpoints (Staff JWT + Pod-Accessible)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/kiosk/experiences` | `list_kiosk_experiences()` | Available experiences |
| POST | `/kiosk/experiences` | `create_kiosk_experience()` | Create experience |
| GET | `/kiosk/experiences/{id}` | `get_kiosk_experience()` | Experience detail |
| PUT | `/kiosk/experiences/{id}` | `update_kiosk_experience()` | Modify |
| DELETE | `/kiosk/experiences/{id}` | `delete_kiosk_experience()` | Remove |
| GET | `/kiosk/settings` | `get_kiosk_settings()` | UI settings |
| PUT | `/kiosk/settings` | `update_kiosk_settings()` | Update settings |
| POST | `/kiosk/pod-launch-experience` | `kiosk_pod_launch_experience()` | Launch game from kiosk |
| POST | `/kiosk/book-multiplayer` | `kiosk_book_multiplayer()` | Multiplayer from kiosk |

---

## Tier 5: Staff/Admin Endpoints (Staff JWT + Pod Source Block)

### Pod Management

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/pods` | `list_pods()` | List all pods |
| POST | `/pods` | `register_pod()` | Register pod |
| POST | `/pods/seed` | `seed_pods()` | Create 8 dummy pods |
| GET | `/pods/{id}` | `get_pod()` | Pod details |
| POST | `/pods/{id}/wake` | `wake_pod()` | Wake-on-LAN |
| POST | `/pods/{id}/shutdown` | `shutdown_pod()` | Graceful shutdown |
| POST | `/pods/{id}/restart` | `restart_pod()` | Restart rc-agent |
| POST | `/pods/{id}/lockdown` | `lockdown_pod()` | Lock pod |
| POST | `/pods/{id}/enable` | `enable_pod()` | Enable pod |
| POST | `/pods/{id}/disable` | `disable_pod()` | Disable (no new bookings) |
| POST | `/pods/{id}/unrestrict` | `unrestrict_pod()` | Remove restrictions |
| POST | `/pods/{id}/freedom` | `freedom_mode_pod()` | Unrestricted test mode |
| POST | `/pods/{id}/screen` | `set_pod_screen()` | Set display mode |
| POST | `/pods/{id}/clear-maintenance` | `clear_maintenance_pod()` | Clear MAINTENANCE_MODE |
| POST | `/pods/{id}/self-test` | `pod_self_test()` | Hardware diagnostics |
| POST | `/pods/{id}/exec` | `ws_exec_pod()` | Execute command on pod |
| POST | `/pods/wake-all` | `wake_all_pods()` | Boot all pods |
| POST | `/pods/shutdown-all` | `shutdown_all_pods()` | Shutdown all |
| POST | `/pods/restart-all` | `restart_all_pods()` | Restart all |
| POST | `/pods/lockdown-all` | `lockdown_all_pods()` | Lockdown all |

### Pod Tuning

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/pods/{pod_id}/assist-state` | `get_pod_assist_state()` | Current assists (ABS, TCS) |
| POST | `/pods/{pod_id}/assists` | `set_pod_assists()` | Update assists |
| POST | `/pods/{pod_id}/transmission` | `set_pod_transmission()` | Auto/manual |
| POST | `/pods/{pod_id}/ffb` | `set_pod_ffb()` | Force feedback settings |
| GET | `/pods/{pod_id}/activity` | `pod_activity()` | Pod activity feed |

### Billing

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/billing/start` | `start_billing()` | Start billing session |
| GET | `/billing/active` | `active_billing_sessions()` | Active sessions |
| GET | `/billing/sessions` | `list_billing_sessions()` | Billing history |
| GET | `/billing/sessions/{id}` | `get_billing_session()` | Session details |
| GET | `/billing/sessions/{id}/events` | `billing_session_events()` | Session events |
| GET | `/billing/sessions/{id}/summary` | `billing_session_summary()` | Cost summary |
| GET | `/billing/sessions/{id}/invoice` | `get_session_invoice()` | GST invoice |
| POST | `/billing/{id}/stop` | `stop_billing()` | End session |
| POST | `/billing/{id}/pause` | `pause_billing()` | Pause |
| POST | `/billing/{id}/resume` | `resume_billing()` | Resume |
| POST | `/billing/{id}/extend` | `extend_billing()` | Add time |
| POST | `/billing/{id}/upgrade` | `upgrade_billing()` | Package upgrade |
| POST | `/billing/{id}/discount` | `apply_billing_discount()` | Staff discount |
| POST | `/billing/{id}/refund` | `refund_billing_session()` | Refund |
| GET | `/billing/{id}/refunds` | `get_billing_refunds()` | Refund history |
| GET | `/billing/{id}/receipt` | `staff_session_receipt()` | Staff receipt view |
| GET | `/billing/split-options/{duration}` | `get_split_options()` | Split session options |
| POST | `/billing/continue-split` | `continue_split()` | Continue after split |

### Games

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/games/launch` | `launch_game()` | Launch game on pod |
| POST | `/games/relaunch/{pod_id}` | `relaunch_game()` | Relaunch with same config |
| POST | `/games/stop` | `stop_game()` | Stop game |
| GET | `/games/catalog` | `games_catalog()` | Available games |
| GET | `/games/active` | `active_games()` | Currently running |
| GET | `/games/history` | `game_launch_history()` | Launch history |
| GET | `/games/pod/{pod_id}` | `pod_game_state()` | Game state on pod |
| GET | `/games/alternatives` | `alternatives_handler()` | Game recommendations |
| GET | `/launch-timeline/recent` | `get_recent_launch_timelines()` | Recent launches |
| GET | `/launch-timeline/{launch_id}` | `get_launch_timeline()` | Launch phase timeline |

### Assetto Corsa LAN

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/ac/presets` | `list_ac_presets()` | AC configs |
| POST | `/ac/presets` | `save_ac_preset()` | Save config |
| GET | `/ac/presets/{id}` | `get_ac_preset()` | Preset detail |
| PUT | `/ac/presets/{id}` | `update_ac_preset()` | Modify |
| DELETE | `/ac/presets/{id}` | `delete_ac_preset()` | Remove |
| POST | `/ac/session/start` | `start_ac_session()` | Start AC session |
| POST | `/ac/session/stop` | `stop_ac_session()` | Stop AC session |
| GET | `/ac/session/active` | `active_ac_session()` | Current AC session |
| GET | `/ac/sessions` | `list_ac_sessions()` | Session history |
| GET | `/ac/sessions/{id}/leaderboard` | `ac_session_leaderboard()` | Session results |
| POST | `/ac/session/{id}/continuous` | `ac_server_set_continuous()` | Infinite laps |
| POST | `/ac/session/retry-pod` | `ac_session_retry_pod()` | Retry on different pod |
| POST | `/ac/session/update-config` | `ac_session_update_config()` | Update mid-session |
| GET | `/ac/content/tracks` | `list_ac_tracks()` | Available tracks |
| GET | `/ac/content/cars` | `list_ac_cars()` | Available cars |

### Drivers

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/drivers` | `list_drivers()` | All drivers |
| POST | `/drivers` | `create_driver()` | Create driver |
| GET | `/drivers/{id}` | `get_driver()` | Driver summary |
| GET | `/drivers/{id}/full-profile` | `get_driver_full_profile()` | Complete profile |
| GET | `/drivers/{id}/rating-history` | `staff_driver_rating_history()` | Rating history |

### Wallet (Staff)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/wallet/transactions` | `all_wallet_transactions()` | All transactions |
| GET | `/wallet/{driver_id}` | `get_wallet()` | Driver wallet |
| POST | `/wallet/{driver_id}/topup` | `topup_wallet()` | Manual topup |
| GET | `/wallet/{driver_id}/transactions` | `wallet_transactions()` | Driver history |
| POST | `/wallet/{driver_id}/debit` | `debit_wallet_manual()` | Manual debit |
| POST | `/wallet/{driver_id}/refund` | `refund_wallet()` | Issue refund |

### Auth Management

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/auth/assign` | `assign_customer()` | Assign customer to pod |
| POST | `/auth/cancel/{id}` | `cancel_assignment()` | Cancel assignment |
| GET | `/auth/pending` | `pending_auth_tokens()` | Pending PIN validations |
| GET | `/auth/pending/{pod_id}` | `pending_auth_token_for_pod()` | Per-pod pending |
| POST | `/auth/start-now` | `start_now()` | Immediately start |
| POST | `/auth/validate-qr` | `validate_qr()` | QR code validation |

### Staff Management

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/staff` | `list_staff()` | All staff |
| POST | `/staff` | `create_staff()` | Hire staff |
| PUT | `/staff/{id}` | `update_staff()` | Update info |
| DELETE | `/staff/{id}` | `delete_staff()` | Deactivate |
| POST | `/staff/{id}/reset-pin` | `reset_staff_pin()` | Reset PIN |
| POST | `/staff/shift-handoff` | `shift_handoff_handler()` | Shift handoff |
| GET | `/staff/shift-briefing` | `shift_briefing_handler()` | Incoming briefing |

### Events & Championships

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/events` | `list_events()` | Hotlap events |
| POST | `/events` | `create_event()` | Create event |
| GET | `/staff/events` | `list_staff_events()` | Staff events |
| POST | `/staff/events` | `create_hotlap_event()` | Create hotlap |
| GET | `/staff/events/{id}` | `get_staff_event()` | Event detail |
| PUT | `/staff/events/{id}` | `update_hotlap_event()` | Modify |
| POST | `/staff/events/{id}/link-session` | `link_group_session_to_event()` | Link session |
| GET | `/staff/championships` | `list_staff_championships()` | Championships |
| POST | `/staff/championships` | `create_championship()` | Create |
| GET | `/staff/championships/{id}` | `get_staff_championship()` | Detail |
| POST | `/staff/championships/{id}/rounds` | `add_championship_round()` | Add round |

### Tournaments

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/tournaments` | `list_tournaments()` | List |
| POST | `/tournaments` | `create_tournament()` | Create |
| GET | `/tournaments/{id}` | `get_tournament()` | Detail |
| PUT | `/tournaments/{id}` | `update_tournament()` | Modify |
| GET | `/tournaments/{id}/registrations` | `tournament_registrations()` | Registered drivers |
| GET | `/tournaments/{id}/matches` | `tournament_matches()` | Match schedule |
| POST | `/tournaments/{id}/generate-bracket` | `generate_bracket()` | Auto bracket |
| POST | `/tournaments/{id}/matches/{match_id}/result` | `record_match_result()` | Record result |

### Debug & Diagnostics

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/debug/db-stats` | `debug_db_stats()` | DB row counts, sizes |
| GET | `/debug/activity` | `debug_activity()` | Global activity log |
| GET | `/debug/playbooks` | `debug_playbooks()` | MI playbooks executed |
| GET | `/debug/incidents` | `list_debug_incidents()` | System incidents |
| POST | `/debug/incidents` | `create_debug_incident()` | Create incident |
| PUT | `/debug/incidents/{id}` | `update_debug_incident()` | Update |
| POST | `/debug/incidents/{id}/apply-fix` | `debug_apply_fix()` | Apply MI fix |
| POST | `/debug/diagnose` | `debug_diagnose()` | Run diagnostics |
| GET | `/debug/pod-events/{pod_id}` | `debug_pod_events()` | Pod event log |
| GET | `/system-events` | `get_events()` | Event archive query |

### Cafe Management

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/cafe/items` | `list_cafe_items()` | Menu items |
| POST | `/cafe/items` | `create_cafe_item()` | Add item |
| PUT | `/cafe/items/{id}` | `update_cafe_item()` | Update |
| DELETE | `/cafe/items/{id}` | `delete_cafe_item()` | Remove |
| POST | `/cafe/items/{id}/toggle` | `toggle_cafe_item_availability()` | Enable/disable |
| POST | `/cafe/items/{id}/image` | `upload_item_image()` | Upload photo |
| POST | `/cafe/items/{id}/restock` | `restock_cafe_item()` | Restock |
| GET | `/cafe/items/low-stock` | `list_low_stock_items()` | Low stock alerts |
| GET | `/cafe/categories` | `list_cafe_categories()` | Categories |
| POST | `/cafe/categories` | `create_cafe_category()` | Create category |
| POST | `/cafe/orders` | `place_cafe_order()` | Place order (staff) |
| POST | `/cafe/import/preview` | `import_preview()` | Preview import |
| POST | `/cafe/import/confirm` | `confirm_import()` | Confirm import |
| GET | `/cafe/promos` | `list_cafe_promos()` | Promotions |
| POST | `/cafe/promos` | `create_cafe_promo()` | Create promo |
| PUT | `/cafe/promos/{id}` | `update_cafe_promo()` | Modify |
| DELETE | `/cafe/promos/{id}` | `delete_cafe_promo()` | Remove |
| POST | `/cafe/promos/{id}/toggle` | `toggle_cafe_promo()` | Enable/disable |
| POST | `/cafe/marketing/broadcast` | `broadcast_promo()` | WhatsApp broadcast |

### Coupons

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/coupons` | `list_coupons()` | All coupons |
| POST | `/coupons` | `create_coupon()` | Create |
| PUT | `/coupons/{id}` | `update_coupon()` | Edit |
| DELETE | `/coupons/{id}` | `delete_coupon()` | Deactivate |

### Presets & Config (Staff Write)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/presets` | `create_preset()` | Create preset |
| PUT | `/presets/{id}` | `update_preset()` | Update |
| DELETE | `/presets/{id}` | `delete_preset()` | Delete |
| POST | `/config/kiosk-allowlist` | `add_kiosk_allowlist_entry()` | Add to allowlist |
| DELETE | `/config/kiosk-allowlist/{name}` | `delete_kiosk_allowlist_entry()` | Remove |
| POST | `/pos/lockdown` | `set_pos_lockdown()` | Lock POS |

### Psychology & Gamification

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/psychology/badges` | `list_badges()` | Available badges |
| GET | `/psychology/badges/{driver_id}` | `driver_badges()` | Earned badges |
| GET | `/psychology/streaks/{driver_id}` | `driver_streak()` | Streak count |
| GET | `/psychology/nudge-queue` | `list_nudge_queue()` | Queued nudges |
| POST | `/psychology/test-nudge` | `test_nudge()` | Test nudge |
| GET | `/staff/{id}/badges` | `staff_badges_list()` | Staff achievements |
| GET | `/staff/gamification/leaderboard` | `staff_gamification_leaderboard()` | Staff ranking |
| GET | `/staff/gamification/kudos` | `staff_kudos_list()` | Kudos list |
| POST | `/staff/gamification/kudos` | `staff_kudos_create()` | Give kudos |
| GET | `/staff/gamification/challenges` | `staff_challenges_list()` | Challenges |
| POST | `/staff/gamification/challenges` | `staff_challenges_create()` | Create challenge |

### HR

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/hr/sjts` | `list_hiring_sjts()` | SJT templates |
| GET | `/hr/sjts/{id}` | `get_hiring_sjt()` | SJT detail |
| GET | `/hr/job-preview` | `list_job_preview()` | Job descriptions |
| GET | `/hr/campaign-templates` | `list_campaign_templates()` | Campaign templates |
| GET | `/hr/nudge-templates` | `list_nudge_templates()` | Message templates |
| GET | `/hr/recognition` | `hr_recognition_data()` | Employee awards |

---

## Tier 5b: Manager+ Endpoints (Staff JWT + Manager Role)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/billing/report/daily` | `daily_billing_report()` | Revenue report |
| GET | `/billing/rates` | `list_billing_rates()` | Current rates |
| POST | `/billing/rates` | `create_billing_rate()` | Add rate |
| PUT | `/billing/rates/{id}` | `update_billing_rate()` | Update rate |
| GET | `/accounting/accounts` | `list_accounts()` | Chart of accounts |
| GET | `/accounting/trial-balance` | `trial_balance()` | Trial balance |
| GET | `/accounting/profit-loss` | `profit_loss()` | P&L |
| GET | `/accounting/balance-sheet` | `balance_sheet()` | Balance sheet |
| GET | `/accounting/journal` | `list_journal_entries()` | GL journal |
| GET | `/audit-log` | `query_audit_log()` | Full audit log |
| GET | `/reconciliation/status` | `reconciliation_status()` | Bank reconciliation |
| POST | `/reconciliation/run` | `reconciliation_run()` | Run reconciliation |
| GET | `/admin/disputes` | `list_disputes_handler()` | All disputes |
| GET | `/admin/disputes/{id}/details` | `dispute_details_handler()` | Dispute detail |
| POST | `/admin/disputes/{id}/resolve` | `resolve_dispute_handler()` | Resolve dispute |
| GET | `/admin/reports/daily-overrides` | `daily_overrides_report()` | Overrides report |
| GET | `/admin/reports/cash-drawer` | `cash_drawer_status()` | Cash reconciliation |
| POST | `/admin/reports/cash-drawer/close` | `cash_drawer_close()` | Close drawer |

---

## Tier 5c: Superadmin-Only Endpoints

### Feature Flags

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/flags` | `list_flags()` | All flags |
| POST | `/flags` | `create_flag()` | Create flag |
| PUT | `/flags/{name}` | `update_flag()` | Toggle flag |

### Config Push

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/config/push` | `push_config()` | Push config to all pods |
| GET | `/config/push/queue` | `get_queue()` | Pending pushes |
| GET | `/config/audit` | `get_audit_log()` | Config change history |
| GET | `/config/pod/{pod_id}` | `get_pod_config_handler()` | Pod AgentConfig |
| POST | `/config/pod/{pod_id}` | `set_pod_config()` | Set pod config |

### Deployment

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/deploy/status` | `deploy_status()` | Deployment status |
| POST | `/deploy/rolling` | `deploy_rolling_handler()` | Rolling update (canary first) |
| POST | `/deploy/{pod_id}` | `deploy_single_pod()` | Deploy to single pod |
| POST | `/fleet/deploy` | `fleet_deploy_handler()` | Fleet-wide deploy |
| GET | `/fleet/deploy/status` | `fleet_deploy_status_handler()` | Deploy progress |
| POST | `/ota/deploy` | `ota_deploy_handler()` | OTA binary update |
| GET | `/ota/status` | `ota_status_handler()` | OTA status |

### Mesh Intelligence

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/mesh/solutions` | `mesh_list_solutions()` | All ML solutions |
| GET | `/mesh/solutions/search` | `mesh_search_solutions()` | Search by symptom |
| GET | `/mesh/solutions/{id}` | `mesh_get_solution()` | Solution detail |
| GET | `/mesh/incidents` | `mesh_list_incidents()` | All incidents |
| GET | `/mesh/stats` | `mesh_stats()` | MI statistics |
| GET | `/mesh/deploy-status` | `mesh_deploy_status()` | Solution deployments |
| GET | `/mesh/audit-check` | `mesh_audit_check()` | Audit mode |
| POST | `/mesh/solutions/{id}/promote` | `mesh_promote_solution()` | Auto-apply |
| POST | `/mesh/solutions/{id}/retire` | `mesh_retire_solution()` | Retire |
| POST | `/mesh/audit-seed` | `mesh_audit_seed()` | Seed findings into KB |

### Policy Engine

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/policy/rules` | `list_rules_handler()` | All rules |
| POST | `/policy/rules` | `create_rule_handler()` | Create rule |
| PUT | `/policy/rules/{id}` | `update_rule_handler()` | Modify |
| DELETE | `/policy/rules/{id}` | `delete_rule_handler()` | Remove |
| GET | `/policy/eval-log` | `list_eval_log_handler()` | Evaluation history |

### Metrics (Superadmin)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/metrics/query` | `query_handler()` | Flexible BI queries |
| GET | `/metrics/names` | `names_handler()` | Available metric names |
| GET | `/metrics/snapshot` | `snapshot_handler()` | Current values |
| GET | `/metrics/launch-stats` | `launch_stats_handler()` | Launch success rates |
| GET | `/metrics/billing-accuracy` | `billing_accuracy_handler()` | Billing reconciliation |
| GET | `/metrics/launch-observability` | `launch_observability_handler()` | Launch phase metrics |
| GET | `/admin/launch-matrix` | `launch_matrix_handler()` | Game-pod compatibility |
| GET | `/admin/combo-list` | `combo_list_handler()` | Combo reliability |
| GET | `/models/evaluations` | `list_model_evaluations()` | AI model evaluations |
| GET | `/models/reputation` | `list_model_reputation()` | Model reputation scores |

### Pipeline & Maintenance

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/pipeline/config` | `pipeline_config_get()` | Autonomous pipeline config |
| POST | `/pipeline/config` | `pipeline_config_set()` | Update pipeline |
| GET | `/pipeline/status` | `pipeline_status()` | Pipeline status |
| POST | `/maintenance/events` | `maintenance_create_event()` | Log event |
| GET | `/maintenance/summary` | `maintenance_summary()` | History summary |
| POST | `/maintenance/tasks` | `maintenance_create_task()` | Create task |
| PATCH | `/maintenance/tasks/{id}` | `maintenance_update_task()` | Update progress |
| GET | `/scheduler/status` | `get_status()` | Scheduler status |
| PUT | `/scheduler/settings` | `update_settings()` | Scheduling rules |
| GET | `/scheduler/analytics` | `get_analytics()` | Analytics |

---

## Tier 6: Service Endpoints (terminal_secret / Sync Auth)

### Cloud Sync

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/sync/changes` | `sync_changes()` | Fetch from cloud |
| POST | `/sync/push` | `sync_push()` | Push to cloud |
| GET | `/sync/health` | `sync_health()` | Sync health |
| POST | `/sync/import-sessions` | `import_sessions()` | Import during failback |

### Action Queue

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/actions` | `create_action()` | Create action |
| GET | `/actions/pending` | `pending_actions()` | Pending actions |
| POST | `/actions/process` | `process_action_endpoint()` | Process action |
| POST | `/actions/{id}/ack` | `ack_action()` | Acknowledge |
| GET | `/actions/history` | `action_history()` | History |

### Terminal (Remote Exec)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| POST | `/terminal/auth` | `terminal_auth()` | Authenticate |
| GET | `/terminal/commands` | `terminal_list()` | List commands |
| POST | `/terminal/commands` | `terminal_submit()` | Submit command |
| GET | `/terminal/commands/pending` | `terminal_pending()` | Pending results |
| POST | `/terminal/commands/{id}/result` | `terminal_result()` | Return result |
| POST | `/terminal/book-multiplayer` | `terminal_book_multiplayer()` | Book multiplayer |
| GET | `/terminal/group-sessions` | `terminal_group_sessions()` | Group sessions |

### Bot (WhatsApp)

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/bot/lookup` | `bot_lookup()` | Driver by phone/name |
| GET | `/bot/pricing` | `bot_pricing()` | Pricing info |
| POST | `/bot/book` | `bot_book()` | Book via bot |
| GET | `/bot/pods-status` | `bot_pods_status()` | Pod availability |
| GET | `/bot/events` | `bot_events()` | Upcoming events |
| GET | `/bot/leaderboard` | `bot_leaderboard()` | Leaderboard |
| GET | `/bot/customer-stats` | `bot_customer_stats()` | Driver stats |
| POST | `/bot/register-lead` | `bot_register_lead()` | New lead |

### Misc Service

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/logs` | `get_server_logs()` | Server logs |
| POST | `/failover/broadcast` | `failover_broadcast()` | Failover command |

---

## WebSocket Endpoints

| Path | Auth | Purpose |
|------|------|---------|
| `/ws/agent` | PSK/JWT | Pod agent <-> server (game commands, billing, state sync) |
| `/ws/dashboard` | None | Browser real-time updates (game state, leaderboards) |
| `/ws/ai-channel` | Service | Mesh intelligence gossip |

### WS Rate Limiting

- Reconnect cooldown: 2s
- Sentinel alert cooldown: 300s per type per pod
- Auth failure lockout: 5 failures in 300s = 300s lockout

### WS Events

#### `ConfigMismatchDetected` (agent -> server -> admin dashboard)

Fired when a pod's running game config differs from the kiosk-requested config. Phase 362.

| Field | Type | Description |
|-------|------|-------------|
| `type` | `"ConfigMismatchDetected"` | Message discriminant |
| `pod_id` | `string` | Pod that reported the mismatch |
| `sim_type` | `string` | Sim adapter (AC, F1_25, iRacing, LMU, AssettoCorsaEvo) |
| `mismatches` | `[string, string, string][]` | Array of `[field_name, expected, actual]` tuples |
| `timestamp` | `string` | ISO 8601 timestamp from pod |

**Alert behavior:** Server logs at WARN, fires WhatsApp alert to staff, broadcasts `DashboardEvent::ConfigMismatch` to admin dashboard, persists to `config_mismatches` table.

**Test endpoint:** `POST /api/v1/internal/test/config-mismatch` (superadmin-only, Phase 367-05 GLD-G-05) fires a synthetic mismatch for E2E verification without a real pod.

---

## Next.js API Routes (Health Only)

| App | Path | Description |
|-----|------|-------------|
| Kiosk | `GET /api/health` | Page availability check |
| Kiosk | `GET /api/health/deep` | Deep health |
| Web | `GET /api/health` | Page availability check |
| Web | `GET /api/health/deep` | Deep health |
| PWA | — | No local API routes (uses Rust backend) |

---

## Authentication

### JWT Claims

```typescript
interface StaffClaims {
  sub: string;    // Staff ID ("admin", employee ID)
  role: string;   // "cashier" | "manager" | "superadmin"
  exp: number;    // UNIX timestamp
  iat: number;    // UNIX timestamp
}
```

### Getting a Token

```bash
# Admin login
curl -X POST http://192.168.31.23:8080/api/v1/auth/admin-login \
  -H "Content-Type: application/json" \
  -d '{"username":"admin","password":"..."}'

# Staff PIN
curl -X POST http://192.168.31.23:8080/api/v1/staff/validate-pin \
  -H "Content-Type: application/json" \
  -d '{"pin":"1234"}'

# Using token
curl -H "Authorization: Bearer <jwt>" \
  http://192.168.31.23:8080/api/v1/pods
```

### Service Auth

```bash
# Inter-service calls use X-Service-Key header
curl -H "X-Service-Key: <key_from_racecontrol.toml>" \
  http://192.168.31.23:8080/api/v1/terminal/commands
```
