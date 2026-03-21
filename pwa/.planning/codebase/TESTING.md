# Testing Patterns

**Analysis Date:** 2026-03-21

## Test Framework

**Status:** Not detected

**Current state:**
- No test framework installed (Jest, Vitest, or Playwright not in `package.json`)
- No test configuration files present (no `jest.config.*`, `vitest.config.*`, `.test.ts`, `.spec.ts` files)
- No test scripts in `package.json` (only `dev`, `build`, `start`, `lint`)
- ESLint configured but no testing rules visible

**Dependencies in package.json:**
```json
{
  "dependencies": {
    "canvas-confetti": "^1.9.4",
    "html5-qrcode": "^2.3.8",
    "next": "16.1.6",
    "react": "19.2.3",
    "react-dom": "19.2.3",
    "recharts": "^3.8.0",
    "sonner": "^2.0.7"
  },
  "devDependencies": {
    "@tailwindcss/postcss": "^4",
    "@types/canvas-confetti": "^1.9.0",
    "@types/node": "^20",
    "@types/react": "^19",
    "@types/react-dom": "^19",
    "tailwindcss": "^4",
    "typescript": "^5"
  }
}
```

No test framework or testing libraries present.

## Manual Testing Approach

Given the absence of automated testing infrastructure, the following patterns are evident in the codebase for error handling and validation:

### Client-Side Error Handling

**Pattern 1: Try-catch-finally in async operations**

From `src/app/login/page.tsx`:
```typescript
const handleSendOtp = async () => {
  if (phone.length < 10) { setError("Enter a valid phone number"); return; }
  setLoading(true);
  setError("");
  try {
    const formatted = phone.startsWith("+") ? phone : `+91${phone}`;
    const res = await api.login(formatted);
    if (res.error) { setError(res.error); } else { setStep("otp"); }
  } catch { setError("Network error. Try again."); }
  finally { setLoading(false); }
};
```

**Testable aspects** (if tests were added):
- Input validation before API call
- Error state management
- Loading state management
- Success/failure branches
- Network error handling

**Pattern 2: Promise.all with per-result handling**

From `src/app/dashboard/page.tsx`:
```typescript
async function load() {
  try {
    const [pRes, sRes, sessRes, gRes] = await Promise.all([
      api.profile(),
      api.stats(),
      api.sessions(),
      api.groupSession(),
    ]);
    if (pRes.driver) setProfile(pRes.driver);
    if (sRes.stats) setStats(sRes.stats);
    if (sessRes.sessions) setRecentSessions(sessRes.sessions.slice(0, 3));
    if (gRes.group_session) {
      const me = pRes.driver;
      const myMember = gRes.group_session.members.find(
        (m) => m.driver_id === me?.id
      );
      if (myMember?.status === "pending") {
        setGroupInvite(gRes.group_session);
      }
    }
  } catch {
    // network error
  } finally {
    setLoading(false);
  }
}
```

**Testable aspects**:
- Parallel data loading
- Conditional data filtering (e.g., pending group invites)
- State updates based on multiple API responses
- Error recovery

### Data Transformation (Pure Functions)

From `src/components/SessionCard.tsx`:
```typescript
function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}m ${s}s`;
}

function formatDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  return d.toLocaleDateString("en-IN", {
    day: "numeric",
    month: "short",
    year: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function statusColor(status: string): string {
  switch (status) {
    case "active":
      return "text-emerald-400";
    case "completed":
      return "text-neutral-400";
    case "ended_early":
      return "text-rp-red";
    case "cancelled":
      return "text-red-400";
    default:
      return "text-rp-grey";
  }
}
```

**Testable aspects**:
- Time formatting with edge cases (0 seconds, large values)
- Date formatting with null handling
- Status-to-color mapping
- Locale-specific formatting (en-IN)

### Form Validation

From `src/app/login/page.tsx`:
```typescript
const handleRegister = async () => {
  if (name.trim().length < 2) { setError("Name must be at least 2 characters"); return; }
  if (!dob) { setError("Date of birth is required"); return; }
  if (!waiverConsent) { setError("You must accept the safety waiver"); return; }
  if (isMinor && !guardianName.trim()) { setError("Guardian name is required for under 18"); return; }
  // ... submit
};
```

**Testable scenarios**:
- Empty/too-short name validation
- Required field validation
- Conditional validation (minor guardian requirements)
- Age calculation from DOB
- Multi-step form flow (phone → OTP → register)

### API Response Validation

From `src/lib/api.ts`:
```typescript
async function fetchApi<T>(
  path: string,
  options: RequestInit = {}
): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options.headers as Record<string, string>),
  };

  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  let data: unknown;
  try {
    data = await res.json();
  } catch {
    throw new Error(`HTTP ${res.status}: non-JSON response`);
  }

  // Auto-logout on JWT auth errors
  if (
    data &&
    typeof data === "object" &&
    "error" in data &&
    typeof (data as Record<string, unknown>).error === "string"
  ) {
    const err = (data as Record<string, unknown>).error as string;
    const hasRedirect = "_clear" in (data as Record<string, unknown>);
    if (err.includes("JWT decode error") || err.includes("Missing Authorization") || err === "session_expired" || hasRedirect) {
      forceLogout();
      return {} as T;
    }
  }

  return data as T;
}
```

**Testable aspects**:
- Token injection in headers
- JWT validation and auto-logout
- Non-JSON response error handling
- Generic type safety
- Session expiration detection

## Test Coverage Gaps

**Critical untested areas:**

| Area | What's not tested | Risk Level |
|------|------------------|------------|
| **Authentication flow** | Multi-step login (phone → OTP → register) with all edge cases | High |
| **Age validation** | Minor/guardian logic, DOB edge cases, leap years | Medium |
| **Form submission** | All validation rules, error states, success flow | High |
| **API error handling** | JWT expiration, network timeouts, malformed responses | High |
| **Data loading** | Promise.all failures, partial response handling, race conditions | Medium |
| **Component rendering** | Conditional rendering (loading, error, empty states) | Medium |
| **Telemetry chart** | Data transformation, chart synchronization, responsive sizing | Low |
| **Navigation** | Protected routes, auth-based redirects, deep linking | High |
| **Confetti logic** | One-per-session gate, sessionStorage cleanup | Low |
| **Wallet state** | Balance calculations, credit conversions (paise to credits) | Medium |

## Integration Test Candidates

If integration tests were to be added, these flows would be critical:

1. **Complete login flow:**
   - Send phone OTP → Verify → Complete registration → Redirect to dashboard
   - Test: missing phone, invalid OTP, registration rejection, session persistence

2. **Session booking and payment:**
   - Browse experiences → Select duration/pod → Checkout → Confirm
   - Test: credit deduction, pricing calculations, pod availability

3. **Group session management:**
   - Create/invite → Pending state → Accept/reject → Race together
   - Test: member list, status transitions, multiplayer state sync

4. **Data loading reliability:**
   - Dashboard loads profile + stats + sessions + invites in parallel
   - One failure should not break others
   - Test: partial failures, retry logic, cache behavior

## Recommendations for Testing Strategy

**Phase 1 - Unit tests for pure functions:**
- Add Vitest (lightweight, ESM-native)
- Test all utility functions in `src/components/` (formatters, status mappers)
- Test all form validation logic from page components
- Test API response parsing in `src/lib/api.ts`

**Phase 2 - Component tests:**
- Use React Testing Library for interactive components
- Test form inputs, validation feedback, error messages
- Test state management and side effects (useEffect hooks)
- Mock API calls with MSW (Mock Service Worker)

**Phase 3 - E2E tests:**
- Use Playwright for critical user flows
- Test authentication complete flow
- Test booking flow
- Test multi-step interactions

**Phase 4 - Coverage targets:**
- Aim for 70%+ coverage on utility functions
- Aim for 60%+ coverage on components
- 100% coverage on auth-related code
- All form validation paths covered

---

*Testing analysis: 2026-03-21*
