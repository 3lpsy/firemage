use crate::{ca::Authority, http, resolve};
use anyhow::{Result, ensure};
use firemage_egress_policy::EgressPolicy;
use firemage_request_signing::SecretResolver;
use std::{
    collections::HashMap,
    net::{Ipv4Addr, SocketAddrV4},
    path::Path,
    sync::Arc,
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::{Mutex, OwnedSemaphorePermit, RwLock, Semaphore},
};
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
mod replacement;
pub use replacement::Replacement;

type ListenerKey = SocketAddrV4;
#[derive(Clone)]
pub struct EgressManager {
    inner: Arc<Inner>,
}
struct Inner {
    ca: Arc<Authority>,
    state: Arc<RwLock<State>>,
    mutation: Mutex<()>,
    shutdown: CancellationToken,
}
#[derive(Default)]
struct State {
    registrations: HashMap<String, Registration>,
    listeners: HashMap<ListenerKey, Listener>,
}
struct Listener {
    cancel: CancellationToken,
    task: tokio::task::JoinHandle<()>,
}
#[derive(Clone)]
struct Registration {
    guest: Ipv4Addr,
    gateway: Ipv4Addr,
    policy: EgressPolicy,
    secrets: Arc<dyn SecretResolver>,
    cancel: CancellationToken,
    connections: Arc<Semaphore>,
    tasks: TaskTracker,
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

impl EgressManager {
    pub async fn new(directory: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(Inner {
                ca: Arc::new(Authority::load(directory.as_ref())?),
                state: Arc::default(),
                mutation: Mutex::new(()),
                shutdown: CancellationToken::new(),
            }),
        })
    }
    pub fn ca_certificate_pem(&self) -> String {
        self.inner.ca.pem()
    }
    pub fn ca_fingerprint(&self) -> String {
        self.inner.ca.fingerprint()
    }
    pub async fn is_registered(&self, id: &str) -> bool {
        self.inner.state.read().await.registrations.contains_key(id)
    }

    pub async fn register(
        &self,
        id: &str,
        guest: Ipv4Addr,
        gateway: Ipv4Addr,
        policy: EgressPolicy,
        secrets: Arc<dyn SecretResolver>,
    ) -> Result<()> {
        policy.validate()?;
        ensure!(
            !id.is_empty()
                && guest != gateway
                && !guest.is_unspecified()
                && !gateway.is_unspecified(),
            "invalid VM egress identity"
        );
        let _mutation = self.inner.mutation.lock().await;
        let mut state = self.inner.state.write().await;
        ensure!(
            !state.registrations.contains_key(id),
            "VM egress already registered"
        );
        ensure!(
            !state.registrations.values().any(|v| v.guest == guest),
            "guest IP already registered"
        );
        let mut pending = Vec::new();
        for port in policy.ports() {
            let key = SocketAddrV4::new(gateway, port);
            if !state.listeners.contains_key(&key) {
                pending.push((key, TcpListener::bind(key).await?));
            }
        }
        state.registrations.insert(
            id.to_owned(),
            Registration {
                guest,
                gateway,
                policy,
                secrets,
                cancel: self.inner.shutdown.child_token(),
                connections: Arc::new(Semaphore::new(128)),
                tasks: TaskTracker::new(),
            },
        );
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
        Ok(())
    }

    pub async fn unregister(&self, id: &str) {
        let _mutation = self.inner.mutation.lock().await;
        let mut state = self.inner.state.write().await;
        let previous = state.registrations.remove(id);
        if let Some(entry) = &previous {
            entry.cancel.cancel();
            entry.tasks.close();
        }
        let unused: Vec<_> = state
            .listeners
            .keys()
            .filter(|key| {
                !state
                    .registrations
                    .values()
                    .any(|v| v.gateway == *key.ip() && v.policy.ports().contains(&key.port()))
            })
            .copied()
            .collect();
        let mut tasks = Vec::new();
        for key in unused {
            if let Some(listener) = state.listeners.remove(&key) {
                listener.cancel.cancel();
                tasks.push(listener.task);
            }
        }
        drop(state);
        if let Some(entry) = previous {
            entry.tasks.wait().await;
        }
        for task in tasks {
            let _ = task.await;
        }
    }
}

async fn listen(
    listener: TcpListener,
    key: ListenerKey,
    state: Arc<RwLock<State>>,
    ca: Arc<Authority>,
    cancel: CancellationToken,
) {
    loop {
        let accepted = tokio::select! { _ = cancel.cancelled() => break, accepted = listener.accept() => accepted };
        let Ok((stream, peer)) = accepted else { break };
        let state = state.read().await;
        let entry = state
            .registrations
            .values()
            .find(|v| {
                std::net::IpAddr::V4(v.guest) == peer.ip()
                    && v.gateway == *key.ip()
                    && v.policy.ports().contains(&key.port())
            })
            .cloned();
        if let Some(entry) = entry
            && let Ok(permit) = entry.connections.clone().try_acquire_owned()
        {
            entry
                .tasks
                .clone()
                .spawn(connection(stream, key.port(), entry, ca.clone(), permit));
        }
    }
}

async fn connection(
    mut stream: TcpStream,
    port: u16,
    entry: Registration,
    ca: Arc<Authority>,
    permit: OwnedSemaphorePermit,
) {
    if let Some(policy) = entry.policy.http.filter(|p| p.port == port) {
        http::serve(
            stream,
            Arc::new(http::Context {
                policy: Arc::new(policy),
                _permit: permit,
                ca,
                cancel: entry.cancel,
                tasks: entry.tasks,
                secrets: entry.secrets,
                upstream: entry.policy.upstream,
            }),
        )
        .await;
    } else if let Some(tunnel) = entry.policy.tunnels.iter().find(|t| t.listen_port == port) {
        let work = async {
            let target = resolve::destination(
                &tunnel.target_host,
                tunnel.target_port,
                &tunnel.allowed_ips,
                false,
            )
            .await?;
            let mut upstream = tokio::time::timeout(
                std::time::Duration::from_secs(10),
                crate::transport::connect(
                    target,
                    entry.policy.upstream.as_ref(),
                    entry.secrets.as_ref(),
                ),
            )
            .await??;
            tokio::io::copy_bidirectional(&mut stream, &mut upstream).await?;
            Ok::<_, anyhow::Error>(())
        };
        tokio::select! {_ = entry.cancel.cancelled() => {}, _ = work => {}}
    }
}
