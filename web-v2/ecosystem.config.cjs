// PM2 ecosystem config for V2 web-v2 host
// Authored: 2026-05-03 ~13:15 IST PART 24 (bono Phase 0.6 activation under D3 user auth)
// Composes-with: PACT-20260503-001 §9 activation gates (all met) / bono COUNTER-PROPOSE A msg=34745
// Standalone build: produced by `npm run build` at /root/racecontrol/web-v2/.next/standalone/server.js
// Port :3500 + basePath /v2 (next.config.ts) — nginx v2.racingpoint.cloud → 127.0.0.1:3500 already configured

module.exports = {
  apps: [
    {
      name: 'racingpoint-web-v2',
      script: '.next/standalone/server.js',
      cwd: '/root/racecontrol/web-v2',
      instances: 1,
      exec_mode: 'fork',
      env: {
        NODE_ENV: 'production',
        PORT: 3500,
        HOSTNAME: '0.0.0.0',
      },
      max_memory_restart: '512M',
      exp_backoff_restart_delay: 100,
      kill_timeout: 5000,
      autorestart: true,
      watch: false,
      max_restarts: 10,
      min_uptime: '10s',
      error_file: '/root/.pm2/logs/racingpoint-web-v2-error.log',
      out_file: '/root/.pm2/logs/racingpoint-web-v2-out.log',
      merge_logs: true,
      time: true,
    },
  ],
};
