//! Audit logging and replay system
//!
//! All execution is fully auditable and replayable.

use crate::models::{ExecutionRecord, Goal};
use crate::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Audit trail storage
pub struct AuditLog {
    records: Arc<RwLock<HashMap<Uuid, ExecutionRecord>>>,
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Store an execution record
    pub async fn record(&self, record: ExecutionRecord) -> Result<Uuid> {
        let audit_id = record.audit_id;
        let mut records = self.records.write().await;
        records.insert(audit_id, record);
        Ok(audit_id)
    }

    /// Retrieve a record by audit ID
    pub async fn get(&self, audit_id: Uuid) -> Result<Option<ExecutionRecord>> {
        let records = self.records.read().await;
        Ok(records.get(&audit_id).cloned())
    }

    /// List all audit IDs for a user (sorted by created_at)
    pub async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        let records = self.records.read().await;

        let mut items: Vec<_> = records
            .iter()
            .filter(|(_, record)| record.user_id == user_id)
            .map(|(id, record)| (*id, record.created_at))
            .collect();

        // Sort by timestamp ascending
        items.sort_by_key(|(_, created_at)| *created_at);

        Ok(items.into_iter().map(|(id, _)| id).collect())
    }

    /// Verify a record's integrity via hash
    pub async fn verify_integrity(&self, audit_id: Uuid) -> Result<bool> {
        let records = self.records.read().await;

        if let Some(record) = records.get(&audit_id) {
            let current_hash = compute_context_hash(&record.goal);
            Ok(current_hash == record.context_snapshot_hash)
        } else {
            Ok(false)
        }
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute SHA256 hash of a goal for integrity verification
/// Uses zero-copy streaming serialization into hasher
pub fn compute_context_hash(goal: &Goal) -> String {
    let mut hasher = Sha256::new();

    // Stream JSON directly into hasher (no intermediate String)
    if serde_json::to_writer(&mut HashWriter(&mut hasher), goal).is_err() {
        return String::new();
    }

    hex::encode(hasher.finalize())
}

/// Adapter to allow writing into Sha256 via std::io::Write
struct HashWriter<'a, H: Digest>(&'a mut H);

impl<'a, H: Digest> Write for HashWriter<'a, H> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.update(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Replay an execution from audit trail
pub async fn replay_execution(
    audit_log: &AuditLog,
    audit_id: Uuid,
) -> Result<Option<ExecutionRecord>> {
    audit_log.get(audit_id).await
}
