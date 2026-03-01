use sqlx::SqlitePool;
use std::collections::HashMap;
use tokio::sync::{broadcast, mpsc, RwLock};

use crate::billing::BillingManager;
use crate::config::Config;
use rc_common::protocol::{CoreToAgentMessage, DashboardEvent};
use rc_common::types::PodInfo;

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub pods: RwLock<HashMap<String, PodInfo>>,
    pub dashboard_tx: broadcast::Sender<DashboardEvent>,
    pub billing: BillingManager,
    /// Map of pod_id -> sender for pushing commands to specific agents
    pub agent_senders: RwLock<HashMap<String, mpsc::Sender<CoreToAgentMessage>>>,
}

impl AppState {
    pub fn new(config: Config, db: SqlitePool) -> Self {
        let (dashboard_tx, _) = broadcast::channel(1024);
        Self {
            config,
            db,
            pods: RwLock::new(HashMap::new()),
            dashboard_tx,
            billing: BillingManager::new(),
            agent_senders: RwLock::new(HashMap::new()),
        }
    }
}
