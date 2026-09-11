use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) struct Ticket {
    pub owner: String,
    pub credential: String,
    pub vm: String,
    pub created: Instant,
}
impl Ticket {
    pub(super) fn is_bound_to(&self, owner: &str, credential: &str, vm: &str) -> bool {
        self.owner == owner && self.credential == credential && self.vm == vm
    }
}
#[derive(Clone)]
pub(crate) struct ShellState {
    pending: Arc<Mutex<HashMap<String, Ticket>>>,
    pub(super) sessions: Arc<tokio::sync::Semaphore>,
}
impl Default for ShellState {
    fn default() -> Self {
        Self {
            pending: Default::default(),
            sessions: Arc::new(tokio::sync::Semaphore::new(32)),
        }
    }
}
impl ShellState {
    pub(super) fn insert(&self, ticket: Ticket) -> anyhow::Result<String> {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        pending.retain(|_, value| value.created.elapsed() < Duration::from_secs(30));
        anyhow::ensure!(pending.len() < 128, "too many pending shell sessions");
        let token = firemage_auth::new_token("shell");
        pending.insert(firemage_auth::token_hash(&token), ticket);
        Ok(token)
    }
    pub(super) fn take(&self, token: &str) -> Option<Ticket> {
        if token.len() > 128 {
            return None;
        }
        self.pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&firemage_auth::token_hash(token))
            .filter(|ticket| ticket.created.elapsed() < Duration::from_secs(30))
    }
}
