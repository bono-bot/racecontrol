//! AC server configuration generation — INI/YAML for multiplayer sessions.
//! Extracted from ac_server.rs (Phase 385, v49.0 Architecture Completion).

use rc_common::types::*;

// ─── INI Generation ──────────────────────────────────────────────────────────

pub fn generate_admin_password(session_name: &str, udp_port: u16) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    session_name.hash(&mut hasher);
    udp_port.hash(&mut hasher);
    format!("rp{:08x}", hasher.finish() as u32)
}

pub fn generate_server_cfg_ini(config: &AcLanSessionConfig) -> String {
    let cars_str = config.cars.join(";");

    // Use plain track name for server_cfg.ini — the vanilla Kunos acServer.exe
    // doesn't support CSP virtual paths (csp/VERSION/../TRACK). CSP version
    // enforcement is handled client-side by Content Manager, not server-side.
    let track_value = config.track.clone();

    let mut ini = format!(
        "[SERVER]\n\
         NAME={}\n\
         CARS={}\n\
         TRACK={}\n\
         CONFIG_TRACK={}\n\
         MAX_CLIENTS={}\n\
         PASSWORD={}\n\
         ADMIN_PASSWORD={}\n\
         UDP_PORT={}\n\
         TCP_PORT={}\n\
         HTTP_PORT={}\n\
         REGISTER_TO_LOBBY=0\n\
         PICKUP_MODE_ENABLED={}\n\
         LOCKED_ENTRY_LIST=0\n\
         LOOP_MODE=1\n\
         SLEEP_TIME=1\n\
         CLIENT_SEND_INTERVAL_HZ=18\n\
         NUM_THREADS=2\n\
         ALLOWED_TYRES_OUT=2\n\
         MAX_BALLAST_KG=300\n\
         QUALIFY_MAX_WAIT_PERC=120\n\
         RACE_PIT_WINDOW_START=0\n\
         RACE_PIT_WINDOW_END=0\n\
         REVERSED_GRID_RACE_POSITIONS=0\n\
         RACE_OVER_TIME=60\n\
         RESULT_SCREEN_TIME=5\n\
         ABS_ALLOWED={}\n\
         TC_ALLOWED={}\n\
         AUTOCLUTCH_ALLOWED={}\n\
         TYRE_BLANKETS_ALLOWED={}\n\
         STABILITY_ALLOWED={}\n\
         FORCE_VIRTUAL_MIRROR={}\n\
         DAMAGE_MULTIPLIER={}\n\
         FUEL_RATE={}\n\
         TYRE_WEAR_RATE={}\n\
         LEGAL_TYRES=\n",
        config.name,
        cars_str,
        track_value,
        config.track_config,
        config.max_clients,
        config.password,
        generate_admin_password(&config.name, config.udp_port),
        config.udp_port,
        config.tcp_port,
        config.http_port,
        if config.pickup_mode { 1 } else { 0 },
        config.abs_allowed,
        config.tc_allowed,
        if config.autoclutch_allowed { 1 } else { 0 },
        if config.tyre_blankets_allowed { 1 } else { 0 },
        if config.stability_allowed { 1 } else { 0 },
        if config.force_virtual_mirror { 1 } else { 0 },
        0, // SAFETY: damage always 0, ignoring config.damage_multiplier
        config.fuel_rate,
        config.tyre_wear_rate,
    );

    // Session blocks
    for session in &config.sessions {
        match session.session_type {
            SessionType::Practice => {
                ini.push_str(&format!(
                    "\n[PRACTICE]\n\
                     NAME={}\n\
                     TIME={}\n\
                     IS_OPEN=1\n\
                     WAIT_TIME={}\n",
                    session.name, session.duration_minutes, session.wait_time_secs
                ));
            }
            SessionType::Qualifying => {
                ini.push_str(&format!(
                    "\n[QUALIFY]\n\
                     NAME={}\n\
                     TIME={}\n\
                     IS_OPEN=1\n\
                     WAIT_TIME={}\n",
                    session.name, session.duration_minutes, session.wait_time_secs
                ));
            }
            SessionType::Race => {
                ini.push_str(&format!(
                    "\n[RACE]\n\
                     NAME={}\n\
                     LAPS={}\n\
                     TIME={}\n\
                     IS_OPEN=1\n\
                     WAIT_TIME={}\n",
                    session.name, session.laps, session.duration_minutes, session.wait_time_secs
                ));
            }
            _ => {}
        }
    }

    // Dynamic track
    let dt = &config.dynamic_track;
    ini.push_str(&format!(
        "\n[DYNAMIC_TRACK]\n\
         SESSION_START={}\n\
         RANDOMNESS={}\n\
         SESSION_TRANSFER={}\n\
         LAP_GAIN={}\n",
        100, // SAFETY: grip always 100%, ignoring dt.session_start
        dt.randomness, dt.session_transfer, dt.lap_gain
    ));

    // Weather
    for (i, w) in config.weather.iter().enumerate() {
        ini.push_str(&format!(
            "\n[WEATHER_{}]\n\
             GRAPHICS={}\n\
             BASE_TEMPERATURE_AMBIENT={}\n\
             BASE_TEMPERATURE_ROAD={}\n\
             VARIATION_AMBIENT={}\n\
             VARIATION_ROAD={}\n\
             WIND_BASE_SPEED_MIN={}\n\
             WIND_BASE_SPEED_MAX={}\n\
             WIND_BASE_DIRECTION={}\n\
             WIND_VARIATION_DIRECTION={}\n",
            i, w.graphics,
            w.base_temperature_ambient, w.base_temperature_road,
            w.variation_ambient, w.variation_road,
            w.wind_base_speed_min, w.wind_base_speed_max,
            w.wind_base_direction, w.wind_variation_direction,
        ));
    }

    ini
}

pub fn generate_entry_list_ini(config: &AcLanSessionConfig) -> String {
    let mut ini = String::new();

    if !config.entries.is_empty() {
        for (i, entry) in config.entries.iter().enumerate() {
            ini.push_str(&format!(
                "[CAR_{}]\n\
                 MODEL={}\n\
                 SKIN={}\n\
                 DRIVERNAME={}\n\
                 GUID={}\n\
                 BALLAST={}\n\
                 RESTRICTOR={}\n\
                 SPECTATOR_MODE=0\n",
                i, entry.car_model, entry.skin, entry.driver_name, entry.guid,
                entry.ballast, entry.restrictor,
            ));
            // AssettoServer AI entry: AI=fixed for AI opponents
            if let Some(ref ai) = entry.ai_mode {
                ini.push_str(&format!("AI={}\n", ai));
            }
            ini.push('\n');
        }
    } else if config.pickup_mode && !config.cars.is_empty() {
        // Generate empty slots alternating across allowed cars
        let num_slots = config.max_clients as usize;
        for i in 0..num_slots {
            let car = &config.cars[i % config.cars.len()];
            ini.push_str(&format!(
                "[CAR_{}]\n\
                 MODEL={}\n\
                 SKIN=\n\
                 DRIVERNAME=\n\
                 GUID=\n\
                 BALLAST=0\n\
                 RESTRICTOR=0\n\
                 SPECTATOR_MODE=0\n\n",
                i, car,
            ));
        }
    }

    ini
}

/// Generate extra_cfg.yml for AssettoServer with AI configuration.
/// Returns empty string if no AI entries are present.
/// When AI entries exist, generates EnableAi: true and optionally AiAggression
/// mapped from the 0-100 AI_LEVEL to a 0.0-1.0 float.
pub fn generate_extra_cfg_yml(config: &AcLanSessionConfig, ai_level: Option<u32>) -> String {
    let has_ai = config.entries.iter().any(|e| e.ai_mode.is_some());
    if !has_ai {
        return String::new();
    }

    let mut yml = String::new();
    yml.push_str("EnableAi: true\n");
    yml.push_str("AiParams:\n");
    yml.push_str("  MaxAiTargetOccupancy: 1.0\n");

    if let Some(level) = ai_level {
        let clamped = level.min(100);
        let fraction = clamped as f64 / 100.0;
        yml.push_str(&format!("  AiAggression: {:.2}\n", fraction));
    }

    yml
}

