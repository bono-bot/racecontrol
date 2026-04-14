use std::sync::Arc;

use uuid::Uuid;

use crate::state::AppState;

// ─── Submodules ────────────────────────────────────────────────────────────

#[path = "accounting_audit.rs"]
mod audit;

#[path = "accounting_reports.rs"]
mod reports;

pub use audit::{log_audit, log_admin_action, snapshot_row};
pub use reports::{get_trial_balance, get_profit_loss, get_balance_sheet};

// ─── Journal Entries (Double-Entry Bookkeeping) ─────────────────────────────

/// A single debit or credit line in a journal entry.
pub struct JournalLine {
    pub account_id: String,
    pub debit_paise: i64,
    pub credit_paise: i64,
}

/// Post a balanced journal entry. Returns the entry ID.
/// Fails if total debits != total credits (the fundamental accounting rule).
pub async fn post_journal_entry(
    state: &Arc<AppState>,
    description: &str,
    reference_type: Option<&str>,
    reference_id: Option<&str>,
    staff_id: Option<&str>,
    lines: &[JournalLine],
) -> Result<String, String> {
    // Validate: at least 2 lines
    if lines.len() < 2 {
        return Err("Journal entry requires at least 2 lines".to_string());
    }

    // Validate: total debits == total credits
    let total_debit: i64 = lines.iter().map(|l| l.debit_paise).sum();
    let total_credit: i64 = lines.iter().map(|l| l.credit_paise).sum();

    if total_debit != total_credit {
        return Err(format!(
            "Entry does not balance: debits={}p, credits={}p",
            total_debit, total_credit
        ));
    }

    if total_debit == 0 {
        return Err("Journal entry cannot be zero".to_string());
    }

    // Validate: each line is either debit or credit, not both
    for line in lines {
        if line.debit_paise > 0 && line.credit_paise > 0 {
            return Err(format!(
                "Line for account {} has both debit and credit",
                line.account_id
            ));
        }
        if line.debit_paise == 0 && line.credit_paise == 0 {
            return Err(format!(
                "Line for account {} has zero amount",
                line.account_id
            ));
        }
    }

    let entry_id = Uuid::new_v4().to_string();

    // C38 audit fix: Idempotency check — reject duplicate reference_ids for wallet transactions
    // Prevents replay attacks that could cause double top-ups or debits
    if let Some(ref_id) = reference_id {
        if !ref_id.is_empty() {
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM journal_entries WHERE reference_id = ? AND reference_type = ? LIMIT 1",
            )
            .bind(ref_id)
            .bind(reference_type.unwrap_or(""))
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();

            if let Some((existing_id,)) = existing {
                tracing::warn!(
                    target: "accounting",
                    reference_id = ref_id,
                    existing_entry = %existing_id,
                    "Idempotency check: duplicate reference_id rejected"
                );
                return Err(format!(
                    "Duplicate transaction: reference_id '{}' already posted as entry {}",
                    ref_id, existing_id
                ));
            }
        }
    }

    // Use a transaction to ensure header + all lines are atomic
    let mut tx = state.db.begin().await
        .map_err(|e| format!("DB error starting transaction: {}", e))?;

    // Insert header
    sqlx::query(
        "INSERT INTO journal_entries (id, description, reference_type, reference_id, staff_id)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&entry_id)
    .bind(description)
    .bind(reference_type)
    .bind(reference_id)
    .bind(staff_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| format!("DB error creating journal entry: {}", e))?;

    // Insert lines
    for line in lines {
        let line_id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO journal_entry_lines (id, journal_entry_id, account_id, debit_paise, credit_paise)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&line_id)
        .bind(&entry_id)
        .bind(&line.account_id)
        .bind(line.debit_paise)
        .bind(line.credit_paise)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("DB error creating journal line: {}", e))?;
    }

    tx.commit().await
        .map_err(|e| format!("DB error committing journal entry: {}", e))?;

    tracing::debug!("Journal entry posted: {} ({}p)", entry_id, total_debit);
    Ok(entry_id)
}

// ─── Convenience: Auto-post journal entries for common wallet operations ─────

/// Post journal entry for a wallet topup.
/// Debit: Cash/Bank/UPI (asset) | Credit: Customer Wallet (liability)
pub async fn post_topup(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    method: &str,
    staff_id: Option<&str>,
    txn_id: Option<&str>,
) {
    let asset_account = match method {
        "topup_cash" => "acc_cash",
        "topup_card" | "topup_upi" | "topup_online" => "acc_bank",
        _ => "acc_cash",
    };

    let desc = format!("Wallet topup ({}) for driver {}", method, driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        staff_id,
        &[
            JournalLine {
                account_id: asset_account.to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a bonus credit.
/// Debit: Promotional Bonuses (expense) | Credit: Customer Wallet (liability)
pub async fn post_bonus(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_id: Option<&str>,
) {
    let desc = format!("Bonus credit for driver {}", driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        None,
        &[
            JournalLine {
                account_id: "acc_promo_bonus".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post GST-separated journal entry for a session debit.
/// Returns (entry_id, net_revenue_paise, gst_paise) on success.
///
/// 18% GST inclusive calculation:
///   net_paise = amount_paise * 100 / 118   (revenue net of GST)
///   gst_paise = amount_paise - net_paise   (18% GST liability)
///
/// 3-line journal entry (balanced):
///   Line 1: Debit  acc_wallet        full amount_paise  (customer pays)
///   Line 2: Credit acc_racing_rev    net_paise          (revenue net of tax)
///   Line 3: Credit acc_gst_payable   gst_paise          (GST liability to remit)
pub async fn post_session_debit_gst(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    session_id: &str,
) -> Result<(String, i64, i64), String> {
    // 18% inclusive GST split (integer arithmetic, no floating point)
    let net_paise = amount_paise * 100 / 118;
    let gst_paise = amount_paise - net_paise;

    let desc = format!("Racing session {} for driver {} (incl. 18% GST)", session_id, driver_id);
    let entry_id = post_journal_entry(
        state,
        &desc,
        Some("billing_session"),
        Some(session_id),
        None,
        &[
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_racing_rev".to_string(),
                debit_paise: 0,
                credit_paise: net_paise,
            },
            JournalLine {
                account_id: "acc_gst_payable".to_string(),
                debit_paise: 0,
                credit_paise: gst_paise,
            },
        ],
    )
    .await?;

    Ok((entry_id, net_paise, gst_paise))
}

/// Post journal entry for a session debit (backward-compatible wrapper).
/// Internally calls post_session_debit_gst for GST-separated accounting.
/// Debit: Customer Wallet (liability decreases) | Credit: Racing Revenue (net) + GST Payable
pub async fn post_session_debit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    session_id: &str,
) {
    if let Err(e) = post_session_debit_gst(state, driver_id, amount_paise, session_id).await {
        tracing::error!("journal entry failed: {}", e);
    }
}

// ─── Invoice Generation (LEGAL-02) ──────────────────────────────────────────

/// Fallback GSTIN used only if config.venue.venue_gstin is missing.
/// Production should always set venue_gstin in racecontrol.toml.
const VENUE_GSTIN_FALLBACK: &str = "36PLACEHOLDER0Z0";

/// SAC code 999692: Amusement and recreation services (sim racing is recreational amusement).
const SAC_CODE: &str = "999692";

/// Generate a GST-compliant invoice for a billing session.
/// Returns the invoice_id on success.
///
/// CGST and SGST are each 9% (intra-state: total 18%).
/// Invoice numbering is monotonically increasing via the invoice_sequence table.
pub async fn generate_invoice(
    state: &Arc<AppState>,
    billing_session_id: &str,
    driver_id: &str,
    driver_name: &str,
    total_paise: i64,
    net_paise: i64,
    gst_paise: i64,
) -> Result<String, String> {
    // Split GST into CGST (9%) and SGST (9%) for intra-state supply
    let cgst_paise = gst_paise / 2;
    let sgst_paise = gst_paise - cgst_paise;

    // Atomically claim the next invoice number
    let row: Option<(i64,)> = sqlx::query_as(
        "UPDATE invoice_sequence SET next_number = next_number + 1 WHERE id = 1
         RETURNING next_number - 1 as current_number",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| format!("DB error fetching invoice number: {}", e))?;

    let invoice_number = row
        .map(|(n,)| n)
        .ok_or_else(|| "invoice_sequence row missing — DB not initialized".to_string())?;

    let invoice_id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO invoices (
            id, invoice_number, billing_session_id, driver_id, driver_name,
            venue_gstin, sac_code, taxable_value_paise, gst_rate_percent,
            cgst_paise, sgst_paise, total_paise
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 18.0, ?, ?, ?)",
    )
    .bind(&invoice_id)
    .bind(invoice_number)
    .bind(billing_session_id)
    .bind(driver_id)
    .bind(driver_name)
    .bind({
        let gstin = &state.config.venue.venue_gstin;
        if gstin.is_empty() || gstin.contains("PLACEHOLDER") {
            tracing::warn!("Invoice generated with fallback GSTIN — set venue.venue_gstin in racecontrol.toml");
            VENUE_GSTIN_FALLBACK
        } else {
            gstin.as_str()
        }
    })
    .bind(SAC_CODE)
    .bind(net_paise)
    .bind(cgst_paise)
    .bind(sgst_paise)
    .bind(total_paise)
    .execute(&state.db)
    .await
    .map_err(|e| format!("DB error creating invoice: {}", e))?;

    tracing::info!(
        invoice_id = %invoice_id,
        invoice_number = invoice_number,
        session_id = billing_session_id,
        total_paise = total_paise,
        "GST invoice generated"
    );

    Ok(invoice_id)
}

/// Post journal entry for a wallet debit (cafe, merchandise, penalty).
/// Debit: Customer Wallet (liability) | Credit: appropriate revenue/expense
pub async fn post_wallet_debit(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    txn_type: &str,
    txn_id: Option<&str>,
) {
    let (credit_account, desc_prefix) = match txn_type {
        "debit_cafe" => ("acc_cafe_rev", "Cafe purchase"),
        "debit_merchandise" => ("acc_merch_rev", "Merchandise purchase"),
        "debit_penalty" => ("acc_penalty_adj", "Penalty charge"),
        _ => ("acc_racing_rev", "Wallet debit"),
    };

    let desc = format!("{} for driver {}", desc_prefix, driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        None,
        &[
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: credit_account.to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a refund.
/// Debit: Refunds (expense) | Credit: Customer Wallet (liability increases)
pub async fn post_refund(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    reference_id: Option<&str>,
) {
    let desc = format!("Refund to driver {}", driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("refund"),
        reference_id,
        None,
        &[
            JournalLine {
                account_id: "acc_refunds".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}

/// Post journal entry for a cash refund (real money returned to customer).
/// REVERSE of post_topup: Dr. Customer Wallet (liability decreases) | Cr. Cash/Bank (asset decreases)
/// Per D-12: cash refund -> Dr. acc_wallet Cr. acc_cash/acc_bank
pub async fn post_cash_refund(
    state: &Arc<AppState>,
    driver_id: &str,
    amount_paise: i64,
    method: &str,
    staff_id: Option<&str>,
    txn_id: Option<&str>,
) {
    // Determine which asset account the cash leaves from
    let asset_account = match method {
        "cash" => "acc_cash",
        "bank" | "card" | "upi" | "online" => "acc_bank",
        _ => "acc_cash",
    };

    let desc = format!("Cash refund ({}) for driver {}", method, driver_id);
    if let Err(e) = post_journal_entry(
        state,
        &desc,
        Some("wallet_transaction"),
        txn_id,
        staff_id,
        &[
            JournalLine {
                account_id: "acc_wallet".to_string(),
                debit_paise: amount_paise,
                credit_paise: 0,
            },
            JournalLine {
                account_id: asset_account.to_string(),
                debit_paise: 0,
                credit_paise: amount_paise,
            },
        ],
    )
    .await {
        tracing::error!("journal entry failed: {}", e);
    }
}
