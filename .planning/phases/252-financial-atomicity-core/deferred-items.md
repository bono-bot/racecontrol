# Deferred Items — 252-financial-atomicity-core

## Out-of-scope pre-existing test failure

**File:** `crates/racecontrol/src/crypto/encryption.rs:218`
**Test:** `load_keys_wrong_length`
**Issue:** `assert!(err.contains("32 bytes"))` but error message says "got 2 bytes" (because "abcd" is 2 bytes, not 32). The assertion should be `assert!(err.contains("bytes"))` or the test input should use a 30-char hex string.
**Status:** Pre-existing before 252-03. Not caused by reconciliation changes.
