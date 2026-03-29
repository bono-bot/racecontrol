# SQLite Encryption at Rest — M16-SEC Implementation Guide

## Problem
All 5+ SQLite databases contain sensitive data and are stored in plaintext:
- racecontrol.db (billing, customers, wallets)
- faces.db (biometric face embeddings)
- people_tracker.db (entry/exit + staff names)
- bot.sqlite (WhatsApp conversations)
- exec-audit.jsonl (command history)

## Solution: SQLCipher

SQLCipher is a drop-in replacement for SQLite that adds AES-256 encryption.

### Rust (racecontrol, rc-sentry-ai)

1. Replace `sqlx` SQLite driver with SQLCipher-enabled version:
   ```toml
   # Cargo.toml
   sqlx = { version = "0.8", features = ["runtime-tokio", "sqlite", "sqlcipher"] }
   ```

2. Set encryption key at connection:
   ```rust
   let key = std::env::var("RC_DB_ENCRYPTION_KEY")
       .unwrap_or_else(|_| {
           tracing::warn!("RC_DB_ENCRYPTION_KEY not set — database NOT encrypted");
           String::new()
       });
   if !key.is_empty() {
       sqlx::query(&format!("PRAGMA key = '{}';", key))
           .execute(&pool)
           .await?;
   }
   ```

3. For rc-sentry-ai (uses rusqlite):
   ```toml
   # Cargo.toml
   rusqlite = { version = "0.32", features = ["bundled-sqlcipher"] }
   ```

### Python (people-tracker)

```python
# pip install sqlcipher3-binary
import sqlcipher3 as sqlite3

conn = sqlite3.connect("data/people_tracker.db")
conn.execute(f"PRAGMA key = '{os.environ['RC_DB_ENCRYPTION_KEY']}'")
```

### Node.js (whatsapp-bot)

```javascript
// npm install better-sqlite3-sqlcipher
const Database = require('better-sqlite3-sqlcipher');
const db = new Database('data/bot.sqlite');
db.pragma(`key = '${process.env.RC_DB_ENCRYPTION_KEY}'`);
```

### Migration Path
1. Export existing data: `sqlite3 racecontrol.db .dump > backup.sql`
2. Create encrypted DB: `sqlcipher encrypted.db "PRAGMA key='...'; .read backup.sql"`
3. Rename: `mv racecontrol.db racecontrol-unencrypted.db.bak`
4. Rename: `mv encrypted.db racecontrol.db`
5. Update RC_DB_ENCRYPTION_KEY in .env.secrets on all machines

### Timeline
- Phase 1: Add sqlcipher dependencies (no code change needed if key is empty = unencrypted)
- Phase 2: Generate key, add to .env.secrets, test with one DB
- Phase 3: Migrate all 5 databases during maintenance window
