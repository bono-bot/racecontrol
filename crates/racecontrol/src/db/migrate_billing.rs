//! Database migrations: billing domain tables.
//!
//! Extracted from db/mod.rs by split-db-migrations.py

use sqlx::sqlite::SqlitePool;

pub(crate) async fn migrate_billing(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pricing_tiers (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            duration_minutes INTEGER NOT NULL,
            price_paise INTEGER NOT NULL,
            is_trial BOOLEAN DEFAULT 0,
            is_active BOOLEAN DEFAULT 1,
            sort_order INTEGER DEFAULT 0,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Seed default pricing tiers
    sqlx::query(
        "INSERT OR IGNORE INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order)
         VALUES
            ('tier_30min', '30 Minutes', 30, 75000, 0, 1),
            ('tier_60min', '1 Hour', 60, 90000, 0, 2),
            ('tier_trial', 'Free Trial', 5, 0, 1, 0)",
    )
    .execute(pool)
    .await?;


    // Per-minute billing rate tiers (DB-driven, admin-configurable)
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_rates (
            id TEXT PRIMARY KEY,
            tier_order INTEGER NOT NULL,
            tier_name TEXT NOT NULL,
            threshold_minutes INTEGER NOT NULL,
            rate_per_min_paise INTEGER NOT NULL,
            is_active BOOLEAN DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now')),
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Seed default billing rates: 25 cr/min (0-30), 20 cr/min (31-60), 15 cr/min (60+)
    sqlx::query(
        "INSERT OR IGNORE INTO billing_rates (id, tier_order, tier_name, threshold_minutes, rate_per_min_paise)
         VALUES
            ('rate_standard', 1, 'Standard', 30, 2500),
            ('rate_extended', 2, 'Extended', 60, 2000),
            ('rate_marathon', 3, 'Marathon', 0, 1500)",
    )
    .execute(pool)
    .await?;


    // Per-minute billing mode columns (Act 2: package vs per_minute pricing)
    let _ = sqlx::query("ALTER TABLE pricing_tiers ADD COLUMN billing_mode TEXT DEFAULT 'package'")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE pricing_tiers ADD COLUMN rate_paise_per_minute INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE pricing_tiers ADD COLUMN minimum_hold_paise INTEGER DEFAULT 10000")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE pricing_tiers ADD COLUMN low_balance_warning_paise INTEGER DEFAULT 5000")
        .execute(pool)
        .await;


    // Seed per-minute tier
    sqlx::query(
        "INSERT OR IGNORE INTO pricing_tiers (id, name, duration_minutes, price_paise, is_trial, sort_order, billing_mode, rate_paise_per_minute, minimum_hold_paise, low_balance_warning_paise)
         VALUES ('tier_per_minute', 'Per Minute', 0, 0, 0, 3, 'per_minute', 2500, 10000, 5000)",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_sessions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            pod_id TEXT NOT NULL,
            pricing_tier_id TEXT NOT NULL REFERENCES pricing_tiers(id),
            allocated_seconds INTEGER NOT NULL,
            driving_seconds INTEGER DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'pending',
            custom_price_paise INTEGER,
            notes TEXT,
            started_at TEXT,
            ended_at TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_events (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            event_type TEXT NOT NULL,
            driving_seconds_at_event INTEGER NOT NULL DEFAULT 0,
            metadata TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // ─── Billing accuracy events (METRICS-03) ─────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_accuracy_events (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            pod_id TEXT NOT NULL,
            sim_type TEXT,
            event_type TEXT NOT NULL,
            launch_command_at TEXT,
            playable_signal_at TEXT,
            billing_start_at TEXT,
            delta_ms INTEGER,
            details TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_accuracy_session ON billing_accuracy_events(session_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_accuracy_pod ON billing_accuracy_events(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_accuracy_created ON billing_accuracy_events(created_at)")
        .execute(pool)
        .await?;


    // ─── Phase 283: Billing audit log (immutable, append-only) ─────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS billing_audit_log (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            pod_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            old_status TEXT NOT NULL,
            new_status TEXT NOT NULL,
            nonce_used TEXT,
            timestamp TEXT NOT NULL DEFAULT (datetime('now')),
            actor TEXT NOT NULL
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_audit_session ON billing_audit_log(session_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_audit_pod ON billing_audit_log(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_audit_timestamp ON billing_audit_log(timestamp)")
        .execute(pool)
        .await?;


    // ─── Recovery events (METRICS-04) ─────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS recovery_events (
            id TEXT PRIMARY KEY,
            pod_id TEXT NOT NULL,
            sim_type TEXT,
            car TEXT,
            track TEXT,
            failure_mode TEXT NOT NULL,
            recovery_action_tried TEXT NOT NULL,
            recovery_outcome TEXT NOT NULL,
            recovery_duration_ms INTEGER,
            error_details TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_recovery_events_pod ON recovery_events(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_recovery_events_failure ON recovery_events(failure_mode)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_recovery_events_created ON recovery_events(created_at)")
        .execute(pool)
        .await?;


    // ─── Minor session tracking on billing_sessions (LEGAL-04/05) ────────────
    // guardian_present: staff confirmed guardian physically present at counter.
    // is_minor_session: computed at billing-start from driver DOB, denormalised for reporting.
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN guardian_present BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN is_minor_session BOOLEAN DEFAULT 0")
        .execute(pool)
        .await;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_driver ON billing_sessions(driver_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_pod ON billing_sessions(pod_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_status ON billing_sessions(status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_events_session ON billing_events(billing_session_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_events_created ON billing_events(created_at)")
        .execute(pool)
        .await?;

    // ─── Link experience to billing session ──────────────────────────────────
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN experience_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN car TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN track TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN sim_type TEXT")
        .execute(pool)
        .await;

    // Act 2: per-minute billing mode tracking per session
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN billing_mode TEXT DEFAULT 'package'")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN rate_paise_per_minute INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN total_debited_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    // ─── Wallet tables ──────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wallets (
            driver_id TEXT PRIMARY KEY REFERENCES drivers(id),
            balance_paise INTEGER NOT NULL DEFAULT 0 CHECK(balance_paise >= 0),
            total_credited_paise INTEGER NOT NULL DEFAULT 0,
            total_debited_paise INTEGER NOT NULL DEFAULT 0,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    // MMA-ITER1-#12 (8/8): Add CHECK constraint for existing tables (ALTER TABLE can't add CHECK in SQLite,
    // but the CREATE TABLE IF NOT EXISTS with CHECK will apply to new databases. For existing DBs,
    // the app-level guard in debit_in_tx() WHERE balance_paise >= amount is the enforcement layer.)


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS wallet_transactions (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            amount_paise INTEGER NOT NULL,
            balance_after_paise INTEGER NOT NULL,
            txn_type TEXT NOT NULL CHECK(txn_type IN (
                'topup_cash','topup_card','topup_upi','topup_online',
                'debit_session','debit_cafe','debit_merchandise','debit_penalty',
                'refund_session','refund_manual',
                'bonus','adjustment'
            )),
            reference_id TEXT,
            notes TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // Wallet indexes
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_txn_driver ON wallet_transactions(driver_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallet_txn_created ON wallet_transactions(created_at)")
        .execute(pool)
        .await?;

    // Add wallet columns to billing_sessions
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN reservation_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN wallet_debit_paise INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN wallet_txn_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN staff_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN split_count INTEGER DEFAULT 1")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN split_duration_minutes INTEGER")
        .execute(pool)
        .await;


    // ─── Discount columns on billing_sessions ────────────────────────────────
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN discount_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN coupon_id TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN original_price_paise INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN discount_reason TEXT")
        .execute(pool)
        .await;


    let _ = sqlx::query("UPDATE billing_sessions SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;

    let _ = sqlx::query("UPDATE pricing_tiers SET updated_at = created_at WHERE updated_at IS NULL")
        .execute(pool)
        .await;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_wallets_updated ON wallets(updated_at)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_pricing_tiers_updated ON pricing_tiers(updated_at)")
        .execute(pool)
        .await?;

    // ─── Refunds ───────────────────────────────────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS refunds (
            id TEXT PRIMARY KEY,
            billing_session_id TEXT NOT NULL,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            amount_paise INTEGER NOT NULL,
            method TEXT NOT NULL CHECK(method IN ('wallet', 'cash', 'upi')),
            reason TEXT NOT NULL,
            notes TEXT,
            staff_id TEXT,
            wallet_txn_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    // ─── Double-Entry Bookkeeping: Chart of Accounts ──────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS accounts (
            id TEXT PRIMARY KEY,
            code INTEGER NOT NULL UNIQUE,
            name TEXT NOT NULL,
            account_type TEXT NOT NULL CHECK(account_type IN ('asset', 'liability', 'equity', 'revenue', 'expense')),
            parent_id TEXT REFERENCES accounts(id),
            description TEXT,
            is_active INTEGER DEFAULT 1,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_accounts_type ON accounts(account_type)")
        .execute(pool)
        .await?;


    // Seed chart of accounts for RacingPoint
    let accounts = [
        // Assets (1xxx)
        ("acc_cash", 1000, "Cash", "asset", "", "Physical cash received"),
        ("acc_bank", 1100, "Bank Account", "asset", "", "Bank deposits (UPI, card)"),
        ("acc_ar", 1200, "Accounts Receivable", "asset", "", "Outstanding customer payments"),
        // Liabilities (2xxx)
        ("acc_wallet", 2000, "Customer Wallet Credits", "liability", "", "Prepaid credits owed to customers"),
        ("acc_gst_payable", 2100, "GST Payable", "liability", "", "Tax collected pending remittance"),
        // Equity (3xxx)
        ("acc_owner_equity", 3000, "Owner's Equity", "equity", "", "Owner investment"),
        ("acc_retained", 3100, "Retained Earnings", "equity", "", "Accumulated net profit"),
        // Revenue (4xxx)
        ("acc_racing_rev", 4000, "Racing Revenue", "revenue", "", "Sim racing session fees"),
        ("acc_cafe_rev", 4100, "Cafe Revenue", "revenue", "", "Food & beverage sales"),
        ("acc_merch_rev", 4200, "Merchandise Revenue", "revenue", "", "Merchandise sales"),
        ("acc_membership_rev", 4300, "Membership Revenue", "revenue", "", "Membership subscription fees"),
        ("acc_tournament_rev", 4400, "Tournament Revenue", "revenue", "", "Tournament entry fees"),
        // Expenses (5xxx)
        ("acc_refunds", 5000, "Refunds Issued", "expense", "", "Session & manual refunds"),
        ("acc_promo_bonus", 5100, "Promotional Bonuses", "expense", "", "Wallet topup bonus credits given"),
        ("acc_cafe_cogs", 5200, "Cafe Cost of Goods", "expense", "", "Food & beverage purchase costs"),
        ("acc_ops_expense", 5300, "Operating Expenses", "expense", "", "Rent, utilities, equipment"),
        ("acc_penalty_adj", 5400, "Penalties & Adjustments", "expense", "", "Manual wallet adjustments"),
    ];

    for (id, code, name, acct_type, _parent, desc) in &accounts {

        sqlx::query(
            "INSERT OR IGNORE INTO accounts (id, code, name, account_type, description)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id)
        .bind(code)
        .bind(name)
        .bind(acct_type)
        .bind(desc)
        .execute(pool)
        .await?;

    }

    // ─── Double-Entry Bookkeeping: Journal Entries ─────────────────────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS journal_entries (
            id TEXT PRIMARY KEY,
            date TEXT NOT NULL DEFAULT (date('now')),
            description TEXT NOT NULL,
            reference_type TEXT,
            reference_id TEXT,
            staff_id TEXT,
            created_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_journal_date ON journal_entries(date)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_journal_ref ON journal_entries(reference_type, reference_id)")
        .execute(pool)
        .await?;


    sqlx::query(
        "CREATE TABLE IF NOT EXISTS journal_entry_lines (
            id TEXT PRIMARY KEY,
            journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id),
            account_id TEXT NOT NULL REFERENCES accounts(id),
            debit_paise INTEGER NOT NULL DEFAULT 0,
            credit_paise INTEGER NOT NULL DEFAULT 0,
            CHECK(debit_paise >= 0 AND credit_paise >= 0),
            CHECK(NOT (debit_paise > 0 AND credit_paise > 0))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_jel_entry ON journal_entry_lines(journal_entry_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_jel_account ON journal_entry_lines(account_id)")
        .execute(pool)
        .await?;


    // ─── LEGAL-02: GST-Compliant Invoices ────────────────────────────────────
    // One invoice per billing session. invoice_number is monotonically increasing
    // via the invoice_sequence table. CGST/SGST split for intra-state supply.
    // venue_gstin: TODO replace placeholder with actual GSTIN from racecontrol.toml
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS invoices (
            id TEXT PRIMARY KEY,
            invoice_number INTEGER NOT NULL UNIQUE,
            billing_session_id TEXT NOT NULL REFERENCES billing_sessions(id),
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            driver_name TEXT NOT NULL,
            venue_gstin TEXT NOT NULL DEFAULT '29AABCU9603R1ZX',
            sac_code TEXT NOT NULL DEFAULT '999692',
            taxable_value_paise INTEGER NOT NULL,
            gst_rate_percent REAL NOT NULL DEFAULT 18.0,
            cgst_paise INTEGER NOT NULL,
            sgst_paise INTEGER NOT NULL,
            total_paise INTEGER NOT NULL,
            created_at TEXT DEFAULT (datetime('now', '+5 hours', '+30 minutes'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_invoices_session ON invoices(billing_session_id)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_invoices_driver ON invoices(driver_id)")
        .execute(pool)
        .await?;


    // Monotonic invoice number sequence — exactly one row (id=1) with a CHECK constraint.
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS invoice_sequence (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            next_number INTEGER NOT NULL DEFAULT 1
        )",
    )
    .execute(pool)
    .await?;


    // Seed the sequence row if it doesn't exist yet.
    sqlx::query("INSERT OR IGNORE INTO invoice_sequence (id, next_number) VALUES (1, 1)")
        .execute(pool)
        .await?;


    // ─── Billing pause-on-disconnect columns ────────────────────────────────
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN pause_count INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN total_paused_seconds INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN last_paused_at TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN refund_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;


    // ─── Session lifecycle autonomy (Phase 49) ────────────────────────────────
    // end_reason: "manual", "orphan_timeout", "crash_limit" — why the session ended
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN end_reason TEXT")
        .execute(pool)
        .await;


    // Phase 82: Add sim_type column for per-game billing rates
    // NULL = universal rate (applies to all games when no game-specific rate exists)
    let _ = sqlx::query("ALTER TABLE billing_rates ADD COLUMN sim_type TEXT")
        .execute(pool)
        .await;

    // ok() / let _ ignores "duplicate column" error on re-run -- idempotent

    // Phase 88 leaderboard sim_type migration is called from mod.rs

    // ─── Debit Intents (wallet debit tracking for reservations) ──────────────
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS debit_intents (
            id TEXT PRIMARY KEY,
            driver_id TEXT NOT NULL REFERENCES drivers(id),
            amount_paise INTEGER NOT NULL,
            reservation_id TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'pending'
                CHECK(status IN ('pending','processing','completed','failed','cancelled')),
            failure_reason TEXT,
            wallet_txn_id TEXT,
            origin TEXT NOT NULL DEFAULT 'cloud',
            created_at TEXT DEFAULT (datetime('now')),
            processed_at TEXT,
            updated_at TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;


    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debit_intents_status ON debit_intents(status)")
        .execute(pool)
        .await?;

    sqlx::query("CREATE INDEX IF NOT EXISTS idx_debit_intents_reservation ON debit_intents(reservation_id)")
        .execute(pool)
        .await?;


    // ─── Phase 251: Timer persistence columns (RESIL-02, FSM-09) ──────────────
    // elapsed_seconds: total wall-clock seconds since session start (persisted every 60s)
    // last_timer_sync_at: ISO timestamp of last successful persist, used for orphan detection
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN elapsed_seconds INTEGER DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN last_timer_sync_at TEXT")
        .execute(pool)
        .await;


    // Index for orphan detection query (Plan 02): find active sessions with stale sync timestamps
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_billing_sessions_status_sync ON billing_sessions(status, last_timer_sync_at)")
        .execute(pool)
        .await?;


    // ─── Phase 252: FATM-02 Idempotency keys for financial operations ─────────
    // Prevents double-charges when the same request is submitted twice (network retry,
    // POS double-tap, etc.). The column is nullable — only set when the client provides
    // a key. The partial unique index enforces uniqueness only on non-NULL values.
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN idempotency_key TEXT")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE wallet_transactions ADD COLUMN idempotency_key TEXT")
        .execute(pool)
        .await;

    // Also add idempotency_key to the refunds table (used by refund_billing_session)
    let _ = sqlx::query("ALTER TABLE refunds ADD COLUMN idempotency_key TEXT")
        .execute(pool)
        .await;

    // ─── Phase 257: BILL-06 Recovery pause seconds ───────────────────────────
    // recovery_pause_seconds: seconds spent in crash-recovery pause (PausedGamePause triggered
    // by a CrashRecovery event). Excluded from billable time in compute_session_cost.
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN recovery_pause_seconds INTEGER DEFAULT 0")
        .execute(pool)
        .await;


    // ─── Phase 257: BILL-08 Customer charge dispute portal ───────────────────
    // dispute_requests: Customer-initiated billing disputes.
    // Lifecycle: pending → approved (triggers refund) | denied (with reason).
    // One active dispute per session enforced via UNIQUE index on billing_session_id
    // WHERE status != 'denied' (denied disputes allow a re-submission).
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS dispute_requests (
          id TEXT PRIMARY KEY,
          billing_session_id TEXT NOT NULL,
          driver_id TEXT NOT NULL,
          reason TEXT NOT NULL,
          status TEXT NOT NULL DEFAULT 'pending',
          created_at TEXT NOT NULL DEFAULT (datetime('now')),
          resolved_at TEXT,
          resolved_by TEXT,
          resolution_reason TEXT,
          refund_amount_paise INTEGER
        )"
    )
    .execute(pool)
    .await
    .map_err(|e| anyhow::anyhow!("Failed to create dispute_requests table: {}", e))?;

    // BILL-08: Unique index — one active dispute per session (denied = re-submittable)
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_dispute_session
         ON dispute_requests(billing_session_id) WHERE status != 'denied'"
    )
    .execute(pool)
    .await;


    // ─── Phase 258: DEPLOY-02 Agent shutdown persistence ────────────────────
    // shutdown_at: timestamp when the agent signalled a graceful shutdown with an
    // active billing session. Used by DEPLOY-04 post-restart recovery to identify
    // sessions that need partial refund processing.
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN shutdown_at TEXT")
        .execute(pool)
        .await;


    // Wallet owner for billing sessions — tracks who gets refunded (parent for linked racers)
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN wallet_owner_id TEXT")
        .execute(pool)
        .await;


    // Acts 1-4: Link billing sessions to visits
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN visit_id TEXT REFERENCES visits(id)")
        .execute(pool)
        .await;


    // Acts 1-4: Per-minute hold tracking on billing sessions
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN hold_paise INTEGER DEFAULT 0")
        .execute(pool)
        .await;


    // ─── v45.0: Credits/Rupees wallet separation (Phase 337) ─────────────────
    // Per D-01: Track total real-money deposits
    let _ = sqlx::query("ALTER TABLE wallets ADD COLUMN rupee_deposited_paise INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    // Per D-02: Track total cash refunds given
    let _ = sqlx::query("ALTER TABLE wallets ADD COLUMN rupee_refunded_paise INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    // Per D-03: Track total promotional credits issued
    let _ = sqlx::query("ALTER TABLE wallets ADD COLUMN bonus_credited_paise INTEGER NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    // Per D-05, D-06: Currency type for each transaction. Default 'credit' makes existing rows valid.
    let _ = sqlx::query("ALTER TABLE wallet_transactions ADD COLUMN currency_type TEXT NOT NULL DEFAULT 'credit'")
        .execute(pool)
        .await;


    // Per D-07: Backfill currency_type on existing transactions
    // topup_cash, topup_card, topup_upi, topup_online → 'rupee'; all others already default to 'credit'
    let _ = sqlx::query(
        "UPDATE wallet_transactions SET currency_type = 'rupee'
         WHERE txn_type IN ('topup_cash', 'topup_card', 'topup_upi', 'topup_online')
         AND currency_type = 'credit'"
    )
        .execute(pool)
        .await;


    // Per D-08: Backfill rupee_deposited_paise from topup transaction sums
    let _ = sqlx::query(
        "UPDATE wallets SET rupee_deposited_paise = COALESCE((
            SELECT SUM(amount_paise) FROM wallet_transactions
            WHERE wallet_transactions.driver_id = wallets.driver_id
            AND wallet_transactions.txn_type IN ('topup_cash', 'topup_card', 'topup_upi', 'topup_online')
            AND wallet_transactions.amount_paise > 0
        ), 0)
        WHERE rupee_deposited_paise = 0"
    )
        .execute(pool)
        .await;


    // Per D-10: Backfill bonus_credited_paise from bonus/adjustment transaction sums
    let _ = sqlx::query(
        "UPDATE wallets SET bonus_credited_paise = COALESCE((
            SELECT SUM(amount_paise) FROM wallet_transactions
            WHERE wallet_transactions.driver_id = wallets.driver_id
            AND wallet_transactions.txn_type IN ('bonus', 'adjustment')
            AND wallet_transactions.amount_paise > 0
        ), 0)
        WHERE bonus_credited_paise = 0"
    )
        .execute(pool)
        .await;


    // Per D-09: rupee_refunded_paise stays 0 for all existing wallets (no cash refund feature existed)
    // No backfill needed — DEFAULT 0 is correct.

    tracing::info!("v45.0 wallet separation columns migrated and backfilled");

    // ─── Phase 363: Data Recording Verification ──────────────────────────────
    // GLD-C-01/C-02: Per-session lap audit + telemetry completeness columns on billing_sessions.
    // Using `let _ =` idiom to silently swallow "duplicate column" on re-run (idempotent).
    // NOTE: All 8 columns go on billing_sessions (NOT sessions) per 363-RESEARCH.md correction.
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_expected INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_actual INTEGER")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_count_flag TEXT DEFAULT 'UNVERIFIED'")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN telemetry_coverage_pct REAL")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN suspect BOOLEAN NOT NULL DEFAULT 0")
        .execute(pool)
        .await;

    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN suspect_reasons TEXT")
        .execute(pool)
        .await;

    // GLD-C-03: CSV fallback receipt timestamp
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN csv_fallback_received_at TEXT")
        .execute(pool)
        .await;

    // GLD-C-04: Grace window deferral timestamp (D-10)
    let _ = sqlx::query("ALTER TABLE billing_sessions ADD COLUMN lap_reject_grace_until TEXT")
        .execute(pool)
        .await;

    // MMA-101 + R4: Prevent duplicate active billing sessions per pod (TOCTOU defense at DB layer)
    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_billing_sessions_pod_active \
         ON billing_sessions(pod_id) \
         WHERE status = 'active' OR status = 'pending' OR status = 'waiting_for_game' \
            OR status = 'paused_manual' OR status = 'paused_disconnect' OR status = 'paused_game_pause'"
    )
        .execute(pool)
        .await?;

    // Phase 252: FATM-02 Idempotency unique indexes
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_billing_sessions_idempotency \
         ON billing_sessions(idempotency_key) WHERE idempotency_key IS NOT NULL",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_wallet_txn_idempotency \
         ON wallet_transactions(idempotency_key) WHERE idempotency_key IS NOT NULL",
    )
    .execute(pool)
    .await;
    let _ = sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_refunds_idempotency \
         ON refunds(idempotency_key) WHERE idempotency_key IS NOT NULL",
    )
    .execute(pool)
    .await;

    Ok(())
}
