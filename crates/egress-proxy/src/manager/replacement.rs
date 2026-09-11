use super::*;
use std::collections::HashSet;

pub struct Replacement {
    pub id: String,
    pub guest: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub policy: Option<EgressPolicy>,
    pub secrets: Arc<dyn SecretResolver>,
}

impl EgressManager {
    /// Bind every new listener before replacing registrations and revoking old connections.
    pub async fn replace_many(&self, replacements: Vec<Replacement>) -> Result<()> {
        let mut ids = HashSet::new();
        let mut guests = HashSet::new();
        for update in &replacements {
            ensure!(
                !update.id.is_empty()
                    && ids.insert(update.id.clone())
                    && guests.insert(update.guest)
                    && update.guest != update.gateway
                    && !update.guest.is_unspecified()
                    && !update.gateway.is_unspecified(),
                "invalid or duplicate VM egress identity"
            );
            if let Some(policy) = &update.policy {
                policy.validate()?;
            }
        }
        let _mutation = self.inner.mutation.lock().await;
        let mut state = self.inner.state.write().await;
        ensure!(
            state
                .registrations
                .iter()
                .all(|(id, entry)| ids.contains(id) || !guests.contains(&entry.guest)),
            "guest IP already registered"
        );
        let mut pending = HashMap::new();
        for update in &replacements {
            for port in update.policy.iter().flat_map(EgressPolicy::ports) {
                let key = SocketAddrV4::new(update.gateway, port);
                if !state.listeners.contains_key(&key) && !pending.contains_key(&key) {
                    pending.insert(key, TcpListener::bind(key).await?);
                }
            }
        }
        let mut previous = Vec::new();
        for update in replacements {
            if let Some(entry) = state.registrations.remove(&update.id) {
                entry.cancel.cancel();
                entry.tasks.close();
                previous.push(entry);
            }
            if let Some(policy) = update.policy {
                state.registrations.insert(
                    update.id,
                    Registration {
                        guest: update.guest,
                        gateway: update.gateway,
                        policy,
                        secrets: update.secrets,
                        cancel: self.inner.shutdown.child_token(),
                        connections: Arc::new(Semaphore::new(128)),
                        tasks: TaskTracker::new(),
                    },
                );
            }
        }
        for (key, listener) in pending {
            let cancel = self.inner.shutdown.child_token();
            let task = tokio::spawn(listen(
                listener,
                key,
                self.inner.state.clone(),
                self.inner.ca.clone(),
                cancel.clone(),
            ));
            state.listeners.insert(key, Listener { cancel, task });
        }
        let unused: Vec<_> = state
            .listeners
            .keys()
            .filter(|key| {
                !state.registrations.values().any(|entry| {
                    entry.gateway == *key.ip() && entry.policy.ports().contains(&key.port())
                })
            })
            .copied()
            .collect();
        let mut closed = Vec::new();
        for key in unused {
            if let Some(listener) = state.listeners.remove(&key) {
                listener.cancel.cancel();
                closed.push(listener.task);
            }
        }
        drop(state);
        for entry in previous {
            entry.tasks.wait().await;
        }
        for task in closed {
            let _ = task.await;
        }
        Ok(())
    }
}
