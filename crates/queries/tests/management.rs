#[tokio::test]
async fn last_admin_and_security_changes_revoke_credentials() {
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let admin = firemage_queries::add_user(&db, "admin".into(), Some("hash".into()), true, None)
        .await
        .unwrap();
    let mut disabled = admin.clone();
    disabled.disabled = true;
    assert!(
        firemage_queries::update_user(&db, disabled.clone())
            .await
            .is_err()
    );
    assert!(firemage_queries::remove_user(&db, &admin.id).await.is_err());
    let other = firemage_queries::add_user(&db, "other".into(), Some("hash".into()), true, None)
        .await
        .unwrap();
    firemage_queries::insert_credential(
        &db,
        &admin.id,
        "token-hash".into(),
        "api",
        "runner",
        firemage_queries::now() + 600,
    )
    .await
    .unwrap();
    firemage_queries::update_user(&db, disabled).await.unwrap();
    assert!(
        firemage_queries::authenticate(&db, "token-hash")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        firemage_queries::api_tokens(&db, &admin.id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(firemage_queries::remove_user(&db, &other.id).await.is_err());
    firemage_queries::remove_user(&db, &admin.id).await.unwrap();
}
