use firemage_queries as queries;
use sea_orm::{ActiveModelTrait, Set};

#[tokio::test]
async fn concurrent_refresh_consumes_session_once_and_expired_tokens_fail() {
    let db = queries::connect("sqlite::memory:").await.unwrap();
    let user = queries::add_user(&db, "owner".into(), None, false, None)
        .await
        .unwrap();
    let credential = queries::insert_credential(
        &db,
        &user.id,
        "original".into(),
        "session",
        "login",
        queries::now() + 3600,
    )
    .await
    .unwrap();
    let (a, b) = tokio::join!(
        queries::rotate(&db, "original", "first".into(), queries::now() + 3600),
        queries::rotate(&db, "original", "second".into(), queries::now() + 3600)
    );
    assert_ne!(a.is_ok(), b.is_ok());
    let winner = if a.is_ok() { "first" } else { "second" };
    assert!(
        queries::authenticate(&db, "original")
            .await
            .unwrap()
            .is_none()
    );
    assert!(queries::authenticate(&db, winner).await.unwrap().is_some());
    let mut expired: firemage_orm::credentials::ActiveModel = credential.into();
    expired.expires_at = Set(queries::now() - 1);
    expired.update(&db).await.unwrap();
    assert!(queries::authenticate(&db, winner).await.unwrap().is_none());
    assert!(
        queries::rotate(&db, winner, "revived".into(), queries::now() + 3600)
            .await
            .is_err()
    );
}
