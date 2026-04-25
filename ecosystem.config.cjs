// PACT-016 (2026-04-25): pm2 hardening to break port-8090 TIME_WAIT crash-loop.
// Pairs with racecontrol.toml:144 tailscale_bind_ip fix (100.71.226.83 → 100.70.177.44).
// Direct-binary target: diag-wrapper.sh's `wait $CHILD_PID` waits on the `while|tee`
// pipeline subprocess, not the racecontrol binary, so pm2 SIGKILL leaves orphan binaries
// holding 8080/8090/8099. PACT-018 (-dirty cleanup) was the planned wrapper removal —
// pulled forward here as part of PACT-016 completion (Uday G33 grant 2026-04-25 ~18:30 IST).
// pm2 captures stdout/stderr in /root/.pm2/logs/racecontrol-{out,error}.log;
// /var/log/racecontrol/exit-trace-*.log will not be appended until wrapper is restored.
module.exports = {
  apps: [
    {
      name: 'racecontrol',
      script: '/root/racecontrol/target/release/racecontrol',
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
