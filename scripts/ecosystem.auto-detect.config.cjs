// PM2 ecosystem config for auto-detect pipeline
// James (Windows): pm2 start scripts/ecosystem.auto-detect.config.cjs
// Bono (Linux):    pm2 start /root/racecontrol/scripts/ecosystem.auto-detect.config.cjs

module.exports = {
  apps: [
    {
      name: "auto-detect",
      script: "bash",
      args: "scripts/auto-detect.sh --mode standard",
      cwd: process.platform === "win32"
        ? "C:\\Users\\bono\\racingpoint\\racecontrol"
        : "/root/racecontrol",
      cron_restart: "30 21 * * *",  // 21:30 UTC = 03:00 IST (James)
      autorestart: false,           // cron-only, don't restart on exit
      watch: false,
      env: {
        AUDIT_PIN: "261121",
        COMMS_PSK: "85d1d06c806b3cc5159676bbed35e29ef0a60661e442a683c2c5a345f2036df0",
        COMMS_URL: "ws://srv1422716.hstgr.cloud:8765",
        TZ: "Asia/Kolkata",
      },
      // PM2 log management
      log_date_format: "YYYY-MM-DD HH:mm:ss Z",
      error_file: process.platform === "win32"
        ? "C:\\Users\\bono\\racingpoint\\racecontrol\\audit\\results\\auto-detect-pm2-error.log"
        : "/root/auto-detect-logs/pm2-error.log",
      out_file: process.platform === "win32"
        ? "C:\\Users\\bono\\racingpoint\\racecontrol\\audit\\results\\auto-detect-pm2-out.log"
        : "/root/auto-detect-logs/pm2-out.log",
      max_size: "10M",
      retain: 5,
    },
  ],
};
