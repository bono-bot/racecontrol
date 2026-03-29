# Staff Off-boarding Checklist — Racing Point eSports

## When a staff member leaves, EVERY item must be completed within 24 hours.

### Access Revocation

- [ ] **Admin PIN** — Rotate venue admin PIN (`bash scripts/rotate-credentials.sh`)
- [ ] **Staff JWT** — Revoke by changing JWT secret (forces all staff to re-login)
- [ ] **WiFi password** — Change if staff knew the password
- [ ] **Tailscale** — Remove their device from Tailscale admin console
- [ ] **SSH keys** — Remove from `~/.ssh/authorized_keys` on server, VPS, James PC
- [ ] **GitHub** — Remove from james-racingpoint organization
- [ ] **Google Workspace** — Remove from racingpoint.in domain (if applicable)
- [ ] **WhatsApp groups** — Remove from staff groups
- [ ] **Evolution API** — Rotate Evolution API key if staff had access
- [ ] **POS system** — Remove staff PIN/account from POS
- [ ] **Physical keys** — Collect venue keys, server room keys
- [ ] **DeskIn remote access** — Change DeskIn password (Spectator PC: 712 906 402)

### Knowledge Transfer

- [ ] Document any systems only this person understood
- [ ] Transfer ownership of any scheduled tasks they managed
- [ ] Check for personal accounts used for venue services (their Gmail for OAuth, etc.)

### Verification

- [ ] Attempt login with old credentials — must fail
- [ ] Check `git log` for any last-minute commits
- [ ] Audit access logs for unusual activity in final 7 days
- [ ] Verify fleet health after access revocation (no broken dependencies on their account)

### DPDP Compliance

- [ ] If staff had access to customer data, document scope and duration
- [ ] Ensure staff's personal device does not retain customer data

---

**Completed by:** _______________
**Date:** _______________
**Verified by:** _______________
