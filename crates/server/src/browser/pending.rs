use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub(super) struct Pending {
    pub nonce: String,
    pub verifier: String,
    pub created: Instant,
}
#[derive(Clone, Default)]
pub(crate) struct BrowserState(Arc<Mutex<HashMap<String, Pending>>>);
impl BrowserState {
    pub(super) fn insert(&self, state: &str, nonce: String, verifier: String) {
        let mut pending = self.0.lock().unwrap_or_else(|error| error.into_inner());
        pending.retain(|_, value| value.created.elapsed() < Duration::from_secs(300));
        if pending.len() >= 64
            && let Some(oldest) = pending
                .iter()
                .min_by_key(|(_, value)| value.created)
                .map(|(key, _)| key.clone())
        {
            pending.remove(&oldest);
        }
        pending.insert(
            firemage_auth::token_hash(state),
            Pending {
                nonce,
                verifier,
                created: Instant::now(),
            },
        );
    }
    pub(super) fn take(&self, state: &str) -> Option<Pending> {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&firemage_auth::token_hash(state))
            .filter(|value| value.created.elapsed() < Duration::from_secs(300))
    }
}
