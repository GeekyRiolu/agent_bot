//! State persistence layer
//!
//! Responsible for storing and loading all state.
//! Currently uses in-memory; can be replaced with Postgres.

use crate::models::{ContextSnapshot, Observation, Plan};
use crate::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Trait for state persistence
#[async_trait::async_trait]
pub trait StateStore: Send + Sync {
    async fn persist_observation(&self, obs: Observation) -> Result<()>;
    async fn persist_plan(&self, plan: &Plan) -> Result<()>;
    async fn load_context(&self, user_id: Uuid) -> Result<ContextSnapshot>;
    async fn load_plan(&self, plan_id: Uuid) -> Result<Option<Plan>>;
    async fn load_observations(&self, plan_id: Uuid) -> Result<Vec<Observation>>;
}

/// In-memory state store for development
pub struct InMemoryStateStore {
    plans: Arc<RwLock<HashMap<Uuid, Plan>>>,
    observations_by_plan: Arc<RwLock<HashMap<Uuid, Vec<Observation>>>>,
    tenant_by_user: Arc<RwLock<HashMap<Uuid, Uuid>>>, // user_id → tenant_id
}

impl InMemoryStateStore {
    pub fn new() -> Self {
        Self {
            plans: Arc::new(RwLock::new(HashMap::new())),
            observations_by_plan: Arc::new(RwLock::new(HashMap::new())),
            tenant_by_user: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStateStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl StateStore for InMemoryStateStore {

    async fn persist_observation(&self, obs: Observation) -> Result<()> {

        // Ensure tenant mapping is stored
        {
            let mut tenants = self.tenant_by_user.write().await;
            tenants.entry(obs.user_id).or_insert(obs.tenant_id);
        }

        let mut observations = self.observations_by_plan.write().await;
        observations
            .entry(obs.plan_id)
            .or_insert_with(Vec::new)
            .push(obs);

        Ok(())
    }

    async fn persist_plan(&self, plan: &Plan) -> Result<()> {

        {
            let mut tenants = self.tenant_by_user.write().await;
            tenants.entry(plan.user_id).or_insert(plan.tenant_id);
        }

        let mut plans = self.plans.write().await;
        plans.insert(plan.plan_id, plan.clone());
        Ok(())
    }

    async fn load_context(&self, user_id: Uuid) -> Result<ContextSnapshot> {

        let tenant_id = {
            let tenants = self.tenant_by_user.read().await;
            tenants
                .get(&user_id)
                .cloned()
                .unwrap_or_else(|| Uuid::nil()) // fallback if not found
        };

        let observations_map = self.observations_by_plan.read().await;

        // Flatten all observations for this user
        let observations: Vec<Observation> = observations_map
            .values()
            .flat_map(|vec| vec.iter())
            .filter(|obs| obs.user_id == user_id)
            .cloned()
            .collect();

        let context = ContextSnapshot {
            snapshot_id: Uuid::new_v4(),
            tenant_id,
            user_id,
            plan_id: Uuid::new_v4(),
            observations,
            portfolio_state: None,
            created_at: chrono::Utc::now(),
            context_hash: "mock_hash".to_string(),
        };

        Ok(context)
    }

    async fn load_plan(&self, plan_id: Uuid) -> Result<Option<Plan>> {
        let plans = self.plans.read().await;
        Ok(plans.get(&plan_id).cloned())
    }

    async fn load_observations(&self, plan_id: Uuid) -> Result<Vec<Observation>> {

        let observations = self.observations_by_plan.read().await;

        Ok(observations
            .get(&plan_id)
            .cloned()
            .unwrap_or_default())
    }
}
