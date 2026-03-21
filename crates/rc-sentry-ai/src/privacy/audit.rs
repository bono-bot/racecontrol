use std::path::Path;

use tokio::sync::mpsc;

/// A single entry in the append-only JSONL audit log.
/// Records all privacy-relevant actions for DPDP Act 2023 compliance.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuditEntry {
    /// ISO 8601 timestamp
    pub timestamp: String,
    /// Action type: "face_detected", "person_deleted", "consent_recorded", "retention_purge"
    pub action: String,
    /// Optional person identifier
    pub person_id: Option<String>,
    /// Who performed the action: "system", "api:<ip>", "admin:<name>"
    pub accessor: String,
    /// Optional human-readable details
    pub details: Option<String>,
}

/// Single-writer audit log using an mpsc channel.
/// Avoids Windows file locking issues by funneling all writes through one task.
pub struct AuditWriter {
    tx: mpsc::Sender<AuditEntry>,
}

impl AuditWriter {
    /// Create a new audit writer that appends JSONL entries to the given path.
    ///
    /// Returns the writer handle and a tokio JoinHandle for the background task.
    /// The writer task opens the file in append mode and writes one JSON line per entry.
    pub fn new(path: String) -> (Self, tokio::task::JoinHandle<()>) {
        let (tx, mut rx) = mpsc::channel::<AuditEntry>(256);

        let handle = tokio::spawn(async move {
            // Ensure parent directory exists
            if let Some(parent) = Path::new(&path).parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    tracing::warn!(error = %e, path = %path, "failed to create audit log directory");
                }
            }

            // Open file in append mode (create if not exists)
            let mut file = match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await
            {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!(error = %e, path = %path, "failed to open audit log file");
                    return;
                }
            };

            use tokio::io::AsyncWriteExt;

            while let Some(entry) = rx.recv().await {
                match serde_json::to_string(&entry) {
                    Ok(json) => {
                        let line = format!("{json}\n");
                        if let Err(e) = file.write_all(line.as_bytes()).await {
                            tracing::warn!(error = %e, "failed to write audit log entry");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "failed to serialize audit log entry");
                    }
                }
            }
        });

        (Self { tx }, handle)
    }

    /// Log an audit entry (non-blocking, best-effort).
    /// Uses `try_send` to avoid blocking the detection pipeline.
    /// If the channel is full, the entry is dropped with a warning.
    pub fn log(&self, entry: AuditEntry) {
        if let Err(e) = self.tx.try_send(entry) {
            tracing::warn!(error = %e, "audit log channel full or closed, entry dropped");
        }
    }

    /// Async version of log -- waits for channel capacity.
    /// Use from async handlers that can afford to wait.
    pub async fn log_async(&self, entry: AuditEntry) {
        if let Err(e) = self.tx.send(entry).await {
            tracing::warn!(error = %e, "audit log channel closed, entry dropped");
        }
    }
}
