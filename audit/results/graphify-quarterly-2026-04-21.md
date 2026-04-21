# Graphify Quarterly Audit — 2026-04-21

Generated: 2026-04-21T22:44:27Z

## 1. graphifyy versions

- James (pip)  : `0.4.23`
- Bono (pipx)  : `UNKNOWN`

## 2. Unified graph

- Path: `/c/Users/bono/racingpoint/racecontrol/graphify-out-unified/graph.json`
- Modified: 2026-04-22 03:59:36.477415200 +0530 (age: 0 days)
- Nodes: ~17682 | Edges: ~49033

## 3. Bono mirror

| repo | size | mtime |
|---|---|---|
| meta-corpus | 1153761 | 2026-04-22 03:58:40.130549300 +0530 |
| racingpoint-api-gateway | 64506 | 2026-04-22 03:58:39.087497800 +0530 |
| racingpoint-cloud-dashboard | 47447 | 2026-04-22 03:58:36.596628200 +0530 |
| racingpoint-dashboard | 78264 | 2026-04-22 03:58:38.268617800 +0530 |
| racingpoint-discord-bot-bono | 55180 | 2026-04-22 03:58:41.028643000 +0530 |
| racingpoint-google | 24212 | 2026-04-22 03:58:37.390156400 +0530 |
| racingpoint-hiring-bot | 90922 | 2026-04-22 03:58:35.710258800 +0530 |
| racingpoint-whatsapp-bot-bono | 320821 | 2026-04-22 03:58:41.923189000 +0530 |

## 4. Tier 3 upstream PR backlog (999.4)

- Drafts: 7 | Opened: 2026-04-22
- Age: 0 days

## Quarterly-audit script

Script: `scripts/graphify-quarterly-audit.sh` (this file). Registered in Windows Task Scheduler as `GraphifyQuarterlyAudit` (90-day cadence). To register manually:

```powershell
schtasks /Create /TN GraphifyQuarterlyAudit /TR "bash C:\\Users\\bono\\racingpoint\\racecontrol\\scripts\\graphify-quarterly-audit.sh" /SC MONTHLY /MO 3 /ST 03:00 /F
```
