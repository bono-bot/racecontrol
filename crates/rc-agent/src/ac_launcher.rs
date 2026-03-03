//! Assetto Corsa full launch sequence for sim racing pods.
//!
//! Flow: Kill AC → Write race.ini → Launch acs.exe → Wait → Restart Conspit Link
//! Requires: CSP gui.ini already patched with FORCE_START=1 (one-time setup)

use std::process::Command;
use std::path::Path;
use std::io::Write;
use anyhow::Result;
use serde::Deserialize;

/// AC launch parameters parsed from the `launch_args` JSON
#[derive(Debug, Clone, Deserialize)]
pub struct AcLaunchParams {
    pub car: String,
    pub track: String,
    #[serde(default = "default_driver")]
    pub driver: String,
    #[serde(default)]
    pub track_config: String,
    #[serde(default = "default_skin")]
    pub skin: String,
}

fn default_driver() -> String { "Driver".to_string() }
fn default_skin() -> String { "00_default".to_string() }

/// Runs the full AC launch sequence. Blocks for ~10 seconds.
pub fn launch_ac(params: &AcLaunchParams) -> Result<u32> {
    tracing::info!("AC launch: {} @ {} for {}", params.car, params.track, params.driver);

    // Step 1: Kill existing AC
    tracing::info!("[1/4] Killing existing AC...");
    let _ = Command::new("taskkill")
        .args(["/IM", "acs.exe", "/F"])
        .output();
    let _ = Command::new("taskkill")
        .args(["/IM", "AssettoCorsa.exe", "/F"])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(2));

    // Step 2: Write race.ini
    tracing::info!("[2/4] Writing race.ini...");
    write_race_ini(params)?;

    // Step 3: Launch acs.exe
    tracing::info!("[3/4] Launching acs.exe...");
    let ac_dir = find_ac_dir()?;
    let acs_exe = ac_dir.join("acs.exe");

    let child = Command::new(&acs_exe)
        .current_dir(&ac_dir)
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to launch acs.exe: {}", e))?;
    let pid = child.id();
    tracing::info!("AC launched with PID {}", pid);

    // Step 4: Wait for AC to load, then restart Conspit Link
    tracing::info!("[4/4] Waiting 8s for AC to load, then restarting Conspit Link...");
    std::thread::sleep(std::time::Duration::from_secs(8));
    restart_conspit_link();

    Ok(pid)
}

/// Write race.ini with AUTOSPAWN=1 and the given car/track/driver
fn write_race_ini(params: &AcLaunchParams) -> Result<()> {
    let race_ini_path = dirs_next::document_dir()
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\Users\User\Documents"))
        .join("Assetto Corsa")
        .join("cfg")
        .join("race.ini");

    let track_config = if params.track_config.is_empty() {
        String::new()
    } else {
        params.track_config.clone()
    };

    let content = format!(r#"[AUTOSPAWN]
ACTIVE=1

[BENCHMARK]
ACTIVE=0

[CAR_0]
SETUP=
SKIN={skin}
MODEL=-
MODEL_CONFIG=
BALLAST=0
RESTRICTOR=0
DRIVER_NAME={driver}
NATIONALITY=IND
NATION_CODE=IND

[DYNAMIC_TRACK]
LAP_GAIN=0
RANDOMNESS=0
SESSION_START=100
SESSION_TRANSFER=100

[GHOST_CAR]
ENABLED=0
FILE=
LOAD=0
PLAYING=0
RECORDING=0
SECONDS_ADVANTAGE=0

[GROOVE]
VIRTUAL_LAPS=10
MAX_LAPS=30
STARTING_LAPS=0

[HEADER]
VERSION=2

[LAP_INVALIDATOR]
ALLOWED_TYRES_OUT=-1

[LIGHTING]
CLOUD_SPEED=0.200
SUN_ANGLE=16
TIME_MULT=1.0

[OPTIONS]
USE_MPH=0

[RACE]
AI_LEVEL=100
CARS=1
CONFIG_TRACK={track_config}
DRIFT_MODE=0
FIXED_SETUP=0
JUMP_START_PENALTY=0
MODEL={car}
MODEL_CONFIG=
PENALTIES=1
RACE_LAPS=0
SKIN={skin}
TRACK={track}

[REMOTE]
ACTIVE=0
GUID=
NAME={driver}
PASSWORD=
SERVER_IP=
SERVER_PORT=
TEAM=

[REPLAY]
ACTIVE=0
FILENAME=

[RESTART]
ACTIVE=0

[SESSION_0]
NAME=Practice
DURATION_MINUTES=60
SPAWN_SET=PIT
TYPE=1
LAPS=0
STARTING_POSITION=1

[TEMPERATURE]
AMBIENT=22
ROAD=28

[WEATHER]
NAME=3_clear

[WIND]
DIRECTION_DEG=0
SPEED_KMH_MAX=0
SPEED_KMH_MIN=0"#,
        car = params.car,
        track = params.track,
        track_config = track_config,
        driver = params.driver,
        skin = params.skin,
    );

    if let Some(parent) = race_ini_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut file = std::fs::File::create(&race_ini_path)?;
    file.write_all(content.as_bytes())?;
    tracing::info!("Wrote race.ini to {}", race_ini_path.display());
    Ok(())
}

/// Find the AC installation directory
fn find_ac_dir() -> Result<std::path::PathBuf> {
    let candidates = [
        r"C:\Program Files (x86)\Steam\steamapps\common\assettocorsa",
        r"C:\Program Files\Steam\steamapps\common\assettocorsa",
        r"D:\SteamLibrary\steamapps\common\assettocorsa",
    ];
    for dir in &candidates {
        let p = Path::new(dir);
        if p.join("acs.exe").exists() {
            return Ok(p.to_path_buf());
        }
    }
    anyhow::bail!("AC installation not found");
}

/// Restart Conspit Link 2.0 so it re-handshakes with AC's telemetry
fn restart_conspit_link() {
    let _ = Command::new("taskkill")
        .args(["/IM", "ConspitLink2.0.exe", "/F"])
        .output();
    std::thread::sleep(std::time::Duration::from_secs(2));

    let conspit_path = r"C:\Program Files (x86)\Conspit Link 2.0\ConspitLink2.0.exe";
    if Path::new(conspit_path).exists() {
        match Command::new(conspit_path).spawn() {
            Ok(_) => tracing::info!("Conspit Link 2.0 restarted"),
            Err(e) => tracing::warn!("Failed to restart Conspit Link: {}", e),
        }
    } else {
        tracing::warn!("Conspit Link not found at {}", conspit_path);
    }
}
