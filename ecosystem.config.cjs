// PACT-016 (2026-04-25): pm2 hardening to break port-8090 TIME_WAIT crash-loop.
// Pairs with racecontrol.toml:144 tailscale_bind_ip fix (100.71.226.83 → 100.70.177.44).
// Direct-binary target: diag-wrapper.sh's `wait $CHILD_PID` waits on the `while|tee`
// pipeline subprocess, not the racecontrol binary, so pm2 SIGKILL leaves orphan binaries
// holding 8080/8090/8099. PACT-018 (-dirty cleanup) was the planned wrapper removal —
// pulled forward here as part of PACT-016 completion (Uday G33 grant 2026-04-25 ~18:30 IST).
//
// PACT-036 (2026-04-25 23:42 IST): re-introduce a THIN exit-trace wrapper to
// capture exit code + signal + stderr tail per cycle. 60+ silent restarts in
// this session leave no forensic trail; wrapper restores it. Avoids PACT-016
// R2 orphan bug by using direct stderr redirection (no pipeline — `wait
// $CHILD` waits on racecontrol PID directly). Diagnostic-only; revert to
// direct-binary path once root cause identified.
// Trace per cycle: /var/log/racecontrol/exit-trace-lite-*.log
module.exports = {
  apps: [
    {
      name: 'racecontrol',
      script: '/root/racecontrol/scripts/exit-trace-lite.sh',
      cwd: '/root/racecontrol',
      min_uptime: '60s',
      max_restarts: 10,
      restart_delay: 5000,
      kill_timeout: 30000,
      autorestart: true,
      exec_mode: 'fork',
      env: {
        RUST_BACKTRACE: 'full',
        RUST_LOG: 'info,racecontrol=debug,racecontrol_crate=debug,deploy_awareness=trace,deploy_awareness_fleet=trace',
      },
    },
  ],
};
