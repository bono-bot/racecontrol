# Incident Response Plan — Racing Point eSports

## Severity Levels

| Level | Definition | Response Time | Example |
|-------|-----------|---------------|---------|
| **SEV-1** | Customer data breach, financial loss, system compromise | **Immediate** (within 15 min) | Database leaked, wallet manipulation, unauthorized access to billing |
| **SEV-2** | Service down affecting customers, security vulnerability exploited | **30 minutes** | All pods offline, blanking screen broken, exec endpoint abused |
| **SEV-3** | Partial service degradation, potential vulnerability found | **2 hours** | Single pod down, cloud sync stale, monitoring gap |
| **SEV-4** | Non-critical issue, improvement opportunity | **Next business day** | Log verbosity, documentation gap, code quality issue |

## Response Procedure

### Step 1: DETECT (Who notices?)
- **Automated:** Fleet monitor alerts (every 5 min), WhatsApp alerting, crash loop detection
- **Manual:** Customer complaint, staff observation, session-start audit
- **External:** Security researcher report, Uday notification

### Step 2: TRIAGE (First 5 minutes)
1. **Classify severity** using table above
2. **Notify:** WhatsApp alert to Uday (all SEVs), Bono (SEV-1/2)
3. **Capture evidence** before making changes:
   - Screenshot/log of the issue
   - `git stash` any in-progress work
   - Note exact time (IST) and affected systems

### Step 3: CONTAIN (Stop the bleeding)
| Scenario | Containment Action |
|----------|-------------------|
| Data breach suspected | Rotate ALL credentials (`bash scripts/rotate-credentials.sh`) |
| Pod compromised | Kill rc-agent + isolate pod from network (disable Tailscale) |
| Server compromised | Stop racecontrol, switch to Bono VPS failover |
| WhatsApp bot abused | `pm2 stop whatsapp-bot` on VPS |
| WiFi attack detected | Disable customer WiFi at router |
| Billing manipulation | Put affected sessions on hold, paper billing fallback |

### Step 4: INVESTIGATE (Cause Elimination Process)
Follow UNIFIED-PROTOCOL.md Phase D: Debug methodology:
1. Document symptom
2. List ALL hypotheses
3. Test & eliminate one by one
4. Fix confirmed cause
5. Verify fix works

### Step 5: RECOVER
1. Apply fix to ALL affected systems (standing rule: fix ALL, not just one)
2. Deploy via standard pipeline (stage-release → deploy-pod/server)
3. Verify via 4-layer shipping gate
4. Clear MAINTENANCE_MODE sentinels if applicable

### Step 6: POST-INCIDENT
1. **LOGBOOK entry** with full timeline
2. **Root cause analysis** in `.planning/audits/`
3. **Standing rule update** if new pattern discovered
4. **Credential rotation** if any credential may have been exposed
5. **Notify affected customers** via WhatsApp if data was breached (DPDP requirement)
6. **MMA audit** on affected code area (post-incident audit per UNIFIED-PROTOCOL.md Phase D.9.1)

## Emergency Contacts

| Person | Channel | When |
|--------|---------|------|
| Uday Singh | WhatsApp (+91 7981264279) | ALL incidents |
| James (AI) | Always available on venue PC | Automated detection |
| Bono (AI) | comms-link relay / SSH | Cloud incidents |

## Credential Rotation After Incident

If ANY credential may have been compromised:
```bash
bash scripts/rotate-credentials.sh
```
This generates new secrets and updates all machines. See CREDENTIAL-ROTATION-POLICY.md.

## Annual Review
This plan must be reviewed and updated every 90 days or after any SEV-1/SEV-2 incident.
Last reviewed: 2026-03-29
