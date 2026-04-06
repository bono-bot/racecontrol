---
phase: 260406-tup
plan: 01
type: execute
wave: 1
depends_on: []
files_modified:
  - pwa/src/app/register/page.tsx
  - pwa/src/lib/api.ts
  - crates/racecontrol/src/api/routes.rs
autonomous: true
requirements: [TUP-01, TUP-02]

must_haves:
  truths:
    - "PWA registration form has a phone number input field"
    - "Phone number is sent to server in the register API call"
    - "Server saves phone number to the drivers table on registration"
    - "Phone validation rejects non-Indian-format numbers"
    - "Cloud-to-venue driver sync delivers new drivers to venue DB"
  artifacts:
    - path: "pwa/src/app/register/page.tsx"
      provides: "Phone input field in registration form"
      contains: "phone"
    - path: "crates/racecontrol/src/api/routes.rs"
      provides: "CustomerRegisterRequest with phone field, UPDATE saves phone"
      contains: "phone"
  key_links:
    - from: "pwa/src/app/register/page.tsx"
      to: "pwa/src/lib/api.ts"
      via: "api.register({ phone: ... })"
      pattern: "phone"
    - from: "pwa/src/lib/api.ts"
      to: "/customer/register"
      via: "POST body includes phone"
      pattern: "phone"
    - from: "crates/racecontrol/src/api/routes.rs"
      to: "drivers table"
      via: "UPDATE drivers SET phone = ?"
      pattern: "phone"
---

<objective>
Fix two bugs: (1) PWA registration form doesn't collect or save phone numbers, and (2) cloud-to-venue driver sync not delivering new drivers due to missing venue config.

Purpose: Phone numbers are critical for customer identification and OTP login. Cloud sync is needed so drivers registered on racingpoint.cloud appear at the venue.
Output: Working phone field in PWA registration + verified cloud sync config on venue server.
</objective>

<execution_context>
@.planning/quick/260406-tup-fix-pwa-registration-missing-phone-and-c/260406-tup-PLAN.md
</execution_context>

<context>
@pwa/src/app/register/page.tsx
@pwa/src/lib/api.ts (lines 728-744)
@crates/racecontrol/src/api/routes.rs (lines 7468-7569 — CustomerRegisterRequest + customer_register handler)
@crates/racecontrol/src/input_validation.rs (validate_phone — 10-digit Indian mobile, accepts +91 prefix)

<interfaces>
From pwa/src/lib/api.ts (line 728-744):
```typescript
register: (data: {
    name: string;
    nickname?: string;
    dob: string;
    email?: string;
    waiver_consent: boolean;
    signature_data?: string;
    guardian_name?: string;
    guardian_phone?: string;
  }) => fetchApi<{ status?: string; driver_id?: string; is_minor?: boolean; error?: string }>(
    "/customer/register", { method: "POST", body: JSON.stringify(data) }
  ),
```

From crates/racecontrol/src/api/routes.rs (line 7468-7479):
```rust
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CustomerRegisterRequest {
    name: String,
    nickname: Option<String>,
    email: Option<String>,
    dob: String,
    waiver_consent: bool,
    signature_data: Option<String>,
    guardian_name: Option<String>,
    guardian_phone: Option<String>,
}
```

From crates/racecontrol/src/input_validation.rs:
```rust
pub fn validate_phone(phone: &str) -> Result<(), String>
// 10-digit Indian mobile (starts with 6-9) or 12-digit with 91 prefix
```

From crates/racecontrol/src/config.rs (line 234-264):
```rust
pub struct CloudConfig {
    pub enabled: bool,
    pub api_url: Option<String>,
    pub sync_interval_secs: u64,
    pub terminal_secret: Option<String>,
    pub comms_link_url: Option<String>,
    pub sync_hmac_key: Option<String>,
    pub origin_id: String,
    // ...
}
```
</interfaces>
</context>

<tasks>

<task type="auto">
  <name>Task 1: Wire phone field through PWA form, API interface, and server handler</name>
  <files>pwa/src/app/register/page.tsx, pwa/src/lib/api.ts, crates/racecontrol/src/api/routes.rs</files>
  <action>
**PWA form (pwa/src/app/register/page.tsx):**
1. Add `const [phone, setPhone] = useState("")` state variable (after the email state, around line 13).
2. Add a phone input field ABOVE the email field (phone is more important than email for this business). Place it after the DOB field (or after the minor/guardian section if minor). Use the same styling pattern as existing fields:
   - Label: "Phone Number *" (required)
   - Input type: `tel`, inputMode: `numeric`
   - onChange: `(e) => setPhone(e.target.value.replace(/\D/g, "").slice(0, 10))` (digits only, max 10)
   - Placeholder: "10-digit mobile number"
   - Same Tailwind classes as other inputs
3. Add phone validation in `handleSubmit`: after the DOB check, add:
   ```
   if (phone.length !== 10 || !/^[6-9]/.test(phone)) {
     setError("Enter a valid 10-digit Indian mobile number");
     return;
   }
   ```
4. Pass `phone` in the `api.register()` call: add `phone: phone.trim()` to the object.
5. Add `phone` to the button's disabled check: `|| phone.length !== 10`

**API interface (pwa/src/lib/api.ts):**
6. Add `phone: string;` to the `register` function's data parameter type (after `name`). It is required, not optional.

**Server handler (crates/racecontrol/src/api/routes.rs):**
7. Add `phone: Option<String>` field to `CustomerRegisterRequest` struct (line ~7473, after `name`). Make it Option because existing clients (kiosk) may not send it yet.
8. Add phone validation in the handler body (after the email validation block around line 7504):
   ```rust
   if let Some(ref phone) = req.phone {
       if let Err(e) = crate::input_validation::validate_phone(phone) {
           return Json(json!({ "error": e }));
       }
   }
   ```
9. Add `phone = ?` to the UPDATE statement (line ~7550). Insert it after `name = ?` in the SET clause. Bind it: `.bind(&req.phone)` in the corresponding position in the bind chain.

**IMPORTANT:** The `CustomerRegisterRequest` uses `#[serde(deny_unknown_fields)]`. The `phone` field MUST be `Option<String>` (not required) so that existing clients without phone don't break with a deserialization error.
  </action>
  <verify>
    <automated>cd C:/Users/bono/racingpoint/racecontrol && cargo check -p racecontrol 2>&1 | tail -5 && cd pwa && npx tsc --noEmit 2>&1 | tail -10</automated>
  </verify>
  <done>
    - PWA registration form shows a phone input field with 10-digit validation
    - API interface includes phone in the register payload
    - Server accepts phone in CustomerRegisterRequest and saves it to the drivers table UPDATE
    - cargo check passes, TypeScript compiles
  </done>
</task>

<task type="auto">
  <name>Task 2: Diagnose and fix cloud-to-venue driver sync config</name>
  <files>(config file on server — runtime diagnosis)</files>
  <action>
1. Read the venue server's racecontrol.toml to check the [cloud] section:
   ```bash
   ssh ADMIN@100.125.108.37 "type C:\RacingPoint\racecontrol.toml" 2>/dev/null
   ```
   If SSH fails, use Tailscale: `ssh ADMIN@100.125.108.37`

2. Check what the [cloud] section contains. The required fields for sync to work:
   - `enabled = true`
   - `api_url = "http://100.70.177.44:8080/api/v1"` (Bono VPS via Tailscale)
   - `terminal_secret = "rp-terminal-2026"` (or whatever the cloud instance uses)
   - `sync_interval_secs = 30` (default)
   - `origin_id = "local"` (default, should be fine)

3. If any of these are missing or wrong, fix the TOML file on the server. Use SSH + a heredoc or echo to append/modify the [cloud] section. NEVER pipe SSH output into the config (standing rule).

4. After fixing config, restart the server to pick up the new config:
   ```bash
   ssh ADMIN@100.125.108.37 "schtasks /Run /TN StartRCTemp"
   ```

5. Verify cloud sync is active by checking server logs:
   ```bash
   ssh ADMIN@100.125.108.37 "curl -s http://localhost:8080/api/v1/sync/health"
   ```

6. Also verify the cloud side responds:
   ```bash
   curl -s http://100.70.177.44:8080/api/v1/health
   ```

7. Test the actual sync by checking if "Uday Singh Test" driver appears on venue DB after a sync cycle (wait 30-60s after restart):
   ```bash
   ssh ADMIN@100.125.108.37 "curl -s http://localhost:8080/api/v1/drivers/search?q=Uday"
   ```

**NOTE:** If venue server is unreachable (Sunday, powered off), document the exact config fix needed and mark as PENDING-DEPLOY. Do NOT mark as complete without runtime verification (standing rule: "resolved = committed + deployed + verified").
  </action>
  <verify>
    <automated>ssh ADMIN@100.125.108.37 "curl -s http://localhost:8080/api/v1/sync/health" 2>&1 | head -5</automated>
  </verify>
  <done>
    - Venue racecontrol.toml has [cloud] section with enabled=true, api_url, terminal_secret
    - Server restarted with new config
    - /sync/health returns active status
    - Cloud-created driver "Uday Singh Test" appears in venue driver search (or documented as pending if server unreachable)
  </done>
</task>

</tasks>

<verification>
1. `cargo check -p racecontrol` passes (phone field compiles)
2. `cd pwa && npx tsc --noEmit` passes (TypeScript compiles)
3. Venue server /sync/health shows active cloud sync
4. PWA register page visually shows phone field (rebuild PWA and check)
</verification>

<success_criteria>
- Phone field visible in PWA registration form
- Phone number saved to drivers table on registration
- Cloud sync active on venue server, delivering cloud-created drivers
</success_criteria>

<output>
After completion, create `.planning/quick/260406-tup-fix-pwa-registration-missing-phone-and-c/260406-tup-SUMMARY.md`
</output>
