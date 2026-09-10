use firemage_runtime::Runtime;
use std::{task::Poll, time::Duration};

#[tokio::test]
async fn waiting_for_vm_does_not_block_independent_resource_locks() {
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let runtime = Runtime::new(db, Default::default());
    let active_vm = runtime.lock("vm").await;
    let mut waiting_monitor = Box::pin(runtime.lock("vm"));
    std::future::poll_fn(|cx| {
        assert!(std::future::Future::poll(waiting_monitor.as_mut(), cx).is_pending());
        Poll::Ready(())
    })
    .await;
    let networks = tokio::time::timeout(Duration::from_millis(250), runtime.lock("networks"))
        .await
        .expect("a waiting VM monitor must not prevent that VM from configuring its network");
    drop(networks);
    drop(active_vm);
    tokio::time::timeout(Duration::from_millis(250), waiting_monitor)
        .await
        .expect("the monitor should acquire the released VM lock");
}
