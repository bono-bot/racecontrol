use sqlx::sqlite::SqlitePool;
use crate::crypto::encryption::FieldCipher;

/// Track the admin PIN hash in system_settings for rotation alerting.
/// Called at startup after migrations. Records the SHA-256 of the current
/// admin_pin_hash so the alerter can detect when it was last changed.
pub async fn check_pin_rotation(
    pool: &SqlitePool,
    config: &crate::config::Config,
) -> Option<String> {
    let pin_hash = match config.auth.admin_pin_hash.as_deref() {
        Some(h) if !h.is_empty() => h,
        _ => {
            tracing::debug!("No admin_pin_hash configured, skipping PIN rotation tracking");
            return None;
        }
    };

    // Hash the current admin_pin_hash to detect changes without storing the actual hash
    use sha2::Digest;
    let current_hash = hex::encode(sha2::Sha256::digest(pin_hash.as_bytes()));

    // Check existing record
    let existing: Option<(String, String)> = sqlx::query_as(
        "SELECT value, updated_at FROM system_settings WHERE key = 'admin_pin_hash_sha256'",
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten();

    match existing {
        Some((stored_hash, updated_at)) if stored_hash == current_hash => {
            // PIN unchanged since last recorded
            tracing::info!(
                "Admin PIN unchanged since {}",
                &updated_at
            );
            Some(updated_at)
        }
        _ => {
            // PIN changed (or first run) -- upsert new hash with current timestamp
            if let Err(e) = sqlx::query(
                "INSERT INTO system_settings (key, value, updated_at) VALUES ('admin_pin_hash_sha256', ?, datetime('now'))
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = datetime('now')",
            )
            .bind(&current_hash)
            .execute(pool)
            .await
            {
                tracing::error!("Failed to upsert admin_pin_hash_sha256: {}", e);
                return None;
            }

            let action = if existing.is_some() { "rotated" } else { "recorded" };
            tracing::info!("Admin PIN hash {} in system_settings", action);

            // Return the new updated_at
            let row: Option<(String,)> = sqlx::query_as(
                "SELECT updated_at FROM system_settings WHERE key = 'admin_pin_hash_sha256'",
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            row.map(|r| r.0)
        }
    }
}

/// Migrate existing plaintext PII to encrypted columns.
/// Idempotent: rows with phone_hash already set are skipped.
/// Processes in batches of 100 rows per transaction.
pub async fn migrate_pii_encryption(db: &SqlitePool, cipher: &FieldCipher) -> Result<(), String> {
    // Count rows needing migration
    let rows: Vec<(String, Option<String>, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT id, phone, email, name, guardian_phone FROM drivers \
             WHERE phone_hash IS NULL AND phone IS NOT NULL"
        )
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to query drivers for PII migration: {e}"))?;

    if rows.is_empty() {
        tracing::info!("PII migration phase 1: no plaintext phones to encrypt");
    } else {
        let total = rows.len();
        let mut migrated = 0usize;

        for chunk in rows.chunks(100) {
            let mut tx = db.begin().await.map_err(|e| format!("Failed to begin transaction: {e}"))?;

            for (id, phone, email, name, guardian_phone) in chunk {
                let phone = match phone {
                    Some(p) if !p.is_empty() => p,
                    _ => continue,
                };

                let phone_hash = cipher.hash_phone(phone);
                let phone_enc = cipher.encrypt_field(phone).map_err(|e| format!("encrypt phone: {e}"))?;

                let email_enc: Option<String> = match email {
                    Some(e) if !e.is_empty() => Some(cipher.encrypt_field(e).map_err(|er| format!("encrypt email: {er}"))?),
                    _ => None,
                };

                let name_enc: Option<String> = match name.as_str() {
                    n if !n.is_empty() => Some(cipher.encrypt_field(n).map_err(|er| format!("encrypt name: {er}"))?),
                    _ => None,
                };

                let (gp_hash, gp_enc): (Option<String>, Option<String>) = match guardian_phone {
                    Some(gp) if !gp.is_empty() => {
                        let h = cipher.hash_phone(gp);
                        let e = cipher.encrypt_field(gp).map_err(|er| format!("encrypt guardian_phone: {er}"))?;
                        (Some(h), Some(e))
                    }
                    _ => (None, None),
                };

                sqlx::query(
                    "UPDATE drivers SET phone_hash=?, phone_enc=?, email_enc=?, name_enc=?, \
                     guardian_phone_hash=?, guardian_phone_enc=?, phone=NULL, email=NULL, \
                     guardian_phone=NULL WHERE id=?"
                )
                .bind(&phone_hash)
                .bind(&phone_enc)
                .bind(&email_enc)
                .bind(&name_enc)
                .bind(&gp_hash)
                .bind(&gp_enc)
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Failed to update driver {id}: {e}"))?;

                migrated += 1;
            }

            tx.commit().await.map_err(|e| format!("Failed to commit transaction: {e}"))?;
        }

        tracing::info!("PII migration phase 1 complete: {migrated}/{total} rows encrypted");
    }

    // Phase 2: Backfill phone_hash for records that have phone_enc but lost phone_hash.
    // This happens when phone was cleared (phone=NULL) before phone_hash was set,
    // leaving orphaned records that send_otp can't find by hash.
    let orphans: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, phone_enc FROM drivers \
         WHERE (phone_hash IS NULL OR phone_hash = '') \
           AND phone_enc IS NOT NULL AND phone_enc != ''"
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to query orphaned phone_enc rows: {e}"))?;

    if !orphans.is_empty() {
        let orphan_count = orphans.len();
        let mut backfilled = 0usize;
        for (id, phone_enc) in &orphans {
            match cipher.decrypt_field(phone_enc) {
                Ok(plaintext_phone) => {
                    let phone_hash = cipher.hash_phone(&plaintext_phone);
                    sqlx::query("UPDATE drivers SET phone_hash = ? WHERE id = ?")
                        .bind(&phone_hash)
                        .bind(id)
                        .execute(db)
                        .await
                        .map_err(|e| format!("Failed to backfill phone_hash for {id}: {e}"))?;
                    backfilled += 1;
                }
                Err(e) => {
                    tracing::warn!("PII migration: cannot decrypt phone_enc for driver {id}: {e}");
                }
            }
        }
        tracing::info!("PII migration phase 2: backfilled phone_hash for {backfilled}/{orphan_count} orphaned records");
    }

    Ok(())
}
