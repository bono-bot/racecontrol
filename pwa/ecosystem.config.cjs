// PM2 ecosystem config for racecontrol-pwa (V1 customer PWA on bono cloud)
// Authored: 2026-05-04 ~15:35 IST PART 28 (bono Phase 0.6 port-conflict regression fix)
// Composes-with: PACT-20260503-001 (V2 host on port 3500) + web-v2/ecosystem.config.cjs (PORT: 3500)
// Standalone build: produced by `npm run build` at /root/racecontrol/pwa/.next/standalone/server.js
//
// Background: Phase 0.6 ACTIVATION 2026-05-03 13:18 IST (commit 65889803) bound
// racingpoint-web-v2 to port 3500 — same port racecontrol-pwa was using before. The
// PWA had no canonical ecosystem.config.cjs (was started ad-hoc via pm2 CLI; PORT=3500
// in pm2 dump only). EADDRINUSE crash-loop ensued (21,333+ restarts as of fix commit).
//
// This file is the canonical pm2 source for racecontrol-pwa. PORT moved to 3501.
// nginx app.racingpoint.cloud `/` proxy_pass updated 3500 → 3501 in same fix.
// V2 host racingpoint-web-v2 unchanged on port 3500 (v2.racingpoint.cloud + app.racingpoint.cloud /v2 still reach V2 host).
//
// Permanence: this file is git-tracked + commit-pinned. pm2 dump.pm2 should be regenerated
// from this config on next pm2 save after `pm2 startOrReload ecosystem.config.cjs`.

module.exports = {
  apps: [
    {
      name: 'racecontrol-pwa',
      script: '.next/standalone/server.js',
      cwd: '/root/racecontrol/pwa',
      instances: 1,
      exec_mode: 'fork',
      env: {
        NODE_ENV: 'production',
        PORT: 3501,
        HOSTNAME: '0.0.0.0',
      },
      max_memory_restart: '512M',
      exp_backoff_restart_delay: 100,
      kill_timeout: 5000,
      autorestart: true,
      watch: false,
      max_restarts: 10,
      min_uptime: '10s',
      error_file: '/root/.pm2/logs/racecontrol-pwa-error.log',
      out_file: '/root/.pm2/logs/racecontrol-pwa-out.log',
      merge_logs: true,
      time: true,
    },
  ],
};
