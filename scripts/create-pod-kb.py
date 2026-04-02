#!/usr/bin/env python3
"""Create a pre-seeded mesh_kb.db for pod deployment.
Outputs to /tmp/mesh_kb_seed.db (or first arg).
"""
import sqlite3, json, hashlib, sys
from datetime import datetime

output_path = sys.argv[1] if len(sys.argv) > 1 else '/tmp/mesh_kb_seed.db'
now = datetime.utcnow().isoformat() + 'Z'

def sha16(s):
    return hashlib.sha256(s.encode()).hexdigest()[:16]

solutions = [
    dict(key='process_crash:conspitlink',
         cause='ConspitLink process multiplication - multiple instances grab HID device',
         fix='taskkill /F /IM ConspitLink.exe before start in bat; enforce singleton',
         typ='deterministic', symp='Steering wheel flickering, Bind failed, 4-11 instances',
         tags=['conspitlink','hid','flicker'], conf=1.0, cnt=40,
         src='v27.0', method='manual_investigation', perm='permanent'),
    dict(key='sentinel_unexpected:MAINTENANCE_MODE',
         cause='MAINTENANCE_MODE sentinel left from crash storm - blocks ALL restarts',
         fix='del C:\\RacingPoint\\MAINTENANCE_MODE & GRACEFUL_RELAUNCH & rcagent-restart-sentinel.txt',
         typ='deterministic', symp='Pod offline but responds to ping, rc-agent not running',
         tags=['maintenance','sentinel','critical'], conf=1.0, cnt=15,
         src='v26.0', method='sre_stuck_state', perm='permanent'),
    dict(key='process_crash:werfault',
         cause='WerFault.exe orphans accumulate after crashes',
         fix='taskkill /F /IM werfault.exe & taskkill /F /IM werreport.exe',
         typ='deterministic', symp='Multiple werfault.exe, memory climbing',
         tags=['orphan','cleanup'], conf=1.0, cnt=100,
         src='tier1', method='deterministic', perm='permanent'),
    dict(key='health_check_fail',
         cause='rc-agent in Session 0 (SYSTEM) - cannot launch GUI',
         fix='Kill rc-agent, RCWatchdog restarts in Session 1. Verify tasklist shows Console.',
         typ='restart', symp='Health OK but edge_process_count=0, blanking broken',
         tags=['session0','gui','blanking','critical'], conf=0.95, cnt=8,
         src='v26.0', method='code_expert_session0', perm='permanent'),
    dict(key='ws_disconnect',
         cause='WS disconnect from server restart - auto-reconnect with backoff',
         fix='Wait 60s for reconnect. If >2min check server then restart agent.',
         typ='config', symp='ws_connected=false, pod http_reachable',
         tags=['websocket','connectivity'], conf=0.9, cnt=50,
         src='v10.0', method='sre_stuck_state', perm='permanent'),
    dict(key='violation_spike',
         cause='Allowlist fetch failed at boot (server down) - empty allowlist',
         fix='Wait for periodic re-fetch (5 min). Or restart rc-agent.',
         typ='config', symp='violation_count_24h very high, all pods affected',
         tags=['allowlist','process-guard','false-positive'], conf=0.95, cnt=8,
         src='v12.0', method='reasoner_absence', perm='permanent'),
    dict(key='game_launch_fail',
         cause='Serde drops unknown JSON fields silently - field name mismatch',
         fix='Verify kiosk fields match AcLaunchParams struct. Check race.ini.',
         typ='config', symp='Game launches but AI wrong or zero opponents',
         tags=['game','serde','silent-failure'], conf=0.9, cnt=5,
         src='v27.0', method='consensus_5model', perm='permanent'),
    dict(key='game_launch_fail',
         cause='GameTracker stuck in Launching - WS dropped mid-delivery',
         fix='Call /games/stop to clear. Retry launch.',
         typ='restart', symp='Launch ok but game never starts, next blocked',
         tags=['game','stuck','gametracker'], conf=0.85, cnt=3,
         src='v26.0', method='sre_stuck_state', perm='workaround'),
    dict(key='display_mismatch',
         cause='NVIDIA Surround dropped after explorer restart',
         fix='NEVER restart explorer on Surround pods. Reboot to restore.',
         typ='manual', symp='Resolution 1024x768, single monitor',
         tags=['display','nvidia','surround'], conf=1.0, cnt=3,
         src='v16.0', method='manual_investigation', perm='permanent'),
    dict(key='display_mismatch',
         cause='Curl output quotes break healer parse - ForceRelaunchBrowser spam',
         fix='Strip quotes from curl stdout in healer script',
         typ='deterministic', symp='Blanking screen flickers every 30s',
         tags=['healer','curl','flicker','blanking'], conf=1.0, cnt=8,
         src='v26.0', method='code_expert_session0', perm='permanent'),
    dict(key='error_spike',
         cause='Billing refund bug: wallet_debit_paise overwritten before calc',
         fix='Save original_debit BEFORE UPDATE. Fixed in 5d1ea000.',
         typ='code_change', symp='Wrong refund on early end, customer loses money',
         tags=['billing','refund','financial','critical'], conf=1.0, cnt=1,
         src='v33.0', method='consensus_5model', perm='permanent'),
    dict(key='process_crash:rc-agent',
         cause='Crash loop from corrupted WMI/COM - ntdll 0xC0000005',
         fix='Reboot pod: shutdown /r /t 5 /f',
         typ='restart', symp='>3 startups in 5min, uptime<30s',
         tags=['crash-loop','ntdll','wmi','reboot'], conf=0.9, cnt=2,
         src='v26.0', method='scanner_enumeration', perm='workaround'),
    dict(key='sentinel_unexpected:OTA_DEPLOYING',
         cause='OTA_DEPLOYING sentinel left from interrupted deploy',
         fix='del C:\\RacingPoint\\OTA_DEPLOYING',
         typ='deterministic', symp='Recovery systems refuse to restart',
         tags=['ota','sentinel','deploy'], conf=1.0, cnt=3,
         src='v22.0', method='deterministic', perm='permanent'),
    dict(key='display_mismatch',
         cause='NVIDIA Overlay, Copilot, Widgets popups over blanking',
         fix='Run disable-popups.ps1 on all pods',
         typ='config', symp='System popups visible over blanking/game',
         tags=['overlay','popup','nvidia','copilot'], conf=1.0, cnt=8,
         src='v16.0', method='manual_investigation', perm='permanent'),
    dict(key='health_check_fail',
         cause='Edge SNSS session files persist old tabs',
         fix='Delete Edge Sessions/* before launching',
         typ='deterministic', symp='Edge opens with old tabs',
         tags=['edge','kiosk','session'], conf=0.95, cnt=8,
         src='v16.0', method='manual_investigation', perm='permanent'),
    dict(key='health_check_fail',
         cause='USB selective suspend disconnects wheelbases',
         fix='powercfg high-performance. Disable USB selective suspend.',
         typ='config', symp='Wheelbase disconnects mid-race',
         tags=['usb','power','wheelbase'], conf=0.95, cnt=8,
         src='v27.0', method='manual_investigation', perm='permanent'),
    dict(key='ws_disconnect',
         cause='Tailscale loses auth on reboot - ForceDaemon not set',
         fix='tailscale up --authkey --unattended. ForceDaemon=true.',
         typ='config', symp='Tailscale NoState after reboot',
         tags=['tailscale','auth','reboot'], conf=1.0, cnt=11,
         src='v10.0', method='sre_stuck_state', perm='permanent'),
    dict(key='health_check_fail',
         cause='SSH banner corrupted TOML config - empty process guard',
         fix='Use scp not ssh cat. Validate first line starts with [',
         typ='config', symp='Process guard flags everything',
         tags=['config','ssh','toml'], conf=1.0, cnt=2,
         src='v12.0', method='manual_investigation', perm='permanent'),
    dict(key='health_check_fail',
         cause='Next.js standalone missing static files - wrong appDir',
         fix='Set outputFileTracingRoot. Verify _next/static/ returns 200.',
         typ='config', symp='Pages unstyled, _next/static 404',
         tags=['nextjs','static','deploy'], conf=1.0, cnt=3,
         src='v17.0', method='manual_investigation', perm='permanent'),
    dict(key='health_check_fail',
         cause='Stale GIT_HASH - cargo cache not invalidated',
         fix='touch crates/<crate>/build.rs before release build',
         typ='deterministic', symp='build_id mismatch after deploy',
         tags=['deploy','build','cargo-cache'], conf=1.0, cnt=5,
         src='v17.0', method='manual_investigation', perm='permanent'),
    dict(key='process_crash:variable_dump',
         cause='VSD Craft Variable_dump.exe crashes on pedal input',
         fix='Kill Variable_dump.exe, disable auto-start',
         typ='deterministic', symp='Crash dumps, game sessions interrupted',
         tags=['usb','vsd','pedal'], conf=0.7, cnt=2,
         src='v26.0', method='scanner_enumeration', perm='workaround'),
    dict(key='process_crash:powershell',
         cause='Watchdog PowerShell multiplication - port 8080 conflict',
         fix='taskkill /F /IM powershell.exe then schtasks /Run /TN StartRCTemp',
         typ='deterministic', symp='Multiple powershell.exe, server wont start',
         tags=['watchdog','powershell','server'], conf=1.0, cnt=6,
         src='v17.0', method='manual_investigation', perm='permanent'),
]

db = sqlite3.connect(output_path)
db.execute('PRAGMA journal_mode=WAL')
db.executescript("""
CREATE TABLE IF NOT EXISTS solutions (
    id TEXT PRIMARY KEY, problem_key TEXT NOT NULL, problem_hash TEXT NOT NULL,
    symptoms TEXT NOT NULL, environment TEXT NOT NULL, root_cause TEXT NOT NULL,
    fix_action TEXT NOT NULL, fix_type TEXT NOT NULL, success_count INTEGER DEFAULT 1,
    fail_count INTEGER DEFAULT 0, confidence REAL DEFAULT 1.0, cost_to_diagnose REAL DEFAULT 0,
    models_used TEXT, source_node TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
    version INTEGER DEFAULT 1, ttl_days INTEGER DEFAULT 90, tags TEXT, diagnosis_method TEXT,
    fix_permanence TEXT DEFAULT 'workaround', recurrence_count INTEGER DEFAULT 0,
    permanent_fix_id TEXT, last_recurrence TEXT, permanent_attempt_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_solutions_hash ON solutions(problem_hash);
CREATE INDEX IF NOT EXISTS idx_solutions_key ON solutions(problem_key);
CREATE TABLE IF NOT EXISTS experiments (
    id TEXT PRIMARY KEY, problem_key TEXT NOT NULL, hypothesis TEXT NOT NULL,
    test_plan TEXT NOT NULL, result TEXT, cost REAL DEFAULT 0,
    node TEXT NOT NULL, created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_experiments_key ON experiments(problem_key);
""")

inserted = 0
for s in solutions:
    sid = 'sol_seed_' + sha16(s['key'] + s['cause'])
    phash = sha16(s['key'] + s['symp'])
    cur = db.execute(
        """INSERT OR IGNORE INTO solutions
        (id, problem_key, problem_hash, symptoms, environment, root_cause, fix_action,
         fix_type, success_count, fail_count, confidence, cost_to_diagnose, models_used,
         source_node, created_at, updated_at, version, ttl_days, tags, diagnosis_method,
         fix_permanence, recurrence_count)
        VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)""",
        (sid, s['key'], phash, json.dumps({'summary': s['symp']}),
         json.dumps({'tags': [f"source:{s['src']}", 'seeded:true']}),
         s['cause'], s['fix'], s['typ'], s['cnt'], 0, s['conf'], 0,
         json.dumps([s['method']]), 'james_seed', now, now, 1, 365,
         json.dumps(s['tags']), s['method'], s['perm'], 0))
    if cur.rowcount > 0:
        inserted += 1

db.commit()
total = db.execute('SELECT COUNT(*) FROM solutions').fetchone()[0]
print(f'Created {output_path}: {inserted} new, {total} total solutions')
db.close()
