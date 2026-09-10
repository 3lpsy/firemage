use crate::Vault;
#[tokio::test]
async fn encrypted_values_are_owner_scoped_and_bound_to_their_names() {
    let dir = tempfile::tempdir().unwrap();
    let db = firemage_queries::connect("sqlite::memory:").await.unwrap();
    let alice = firemage_queries::add_user(&db, "alice".into(), None, true, None)
        .await
        .unwrap();
    let bob = firemage_queries::add_user(&db, "bob".into(), None, true, None)
        .await
        .unwrap();
    let vault = Vault::new(db.clone(), dir.path()).await.unwrap();
    vault
        .put(&alice.id, "key", "sensitive-value")
        .await
        .unwrap();
    assert_eq!(
        vault.resolve(&alice.id, "key").await.unwrap(),
        "sensitive-value"
    );
    assert!(vault.resolve(&bob.id, "key").await.is_err());
    assert!(!vault.delete(&bob.id, "key").await.unwrap());
    assert!(vault.list(&bob.id).await.unwrap().is_empty());
    let row = firemage_queries::secret(&db, &alice.id, "key")
        .await
        .unwrap();
    assert!(!row.ciphertext.contains("sensitive-value"));
    firemage_queries::put_secret(&db, &bob.id, "key", row.ciphertext.clone())
        .await
        .unwrap();
    assert!(vault.resolve(&bob.id, "key").await.is_err());
    firemage_queries::put_secret(&db, &alice.id, "other", row.ciphertext)
        .await
        .unwrap();
    assert!(vault.resolve(&alice.id, "other").await.is_err());
    vault.put(&alice.id, "key", "new-value").await.unwrap();
    let reopened = Vault::new(db, dir.path()).await.unwrap();
    assert_eq!(
        reopened.resolve(&alice.id, "key").await.unwrap(),
        "new-value"
    );
    assert!(reopened.delete(&alice.id, "key").await.unwrap());
}
#[test]
fn encryption_key_rejects_symlinks_and_public_permissions() {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let dir = tempfile::tempdir().unwrap();
    let key = crate::key::load(dir.path()).unwrap();
    assert_eq!(key.len(), 32);
    std::fs::set_permissions(
        dir.path().join("secrets.key"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    assert!(crate::key::load(dir.path()).is_err());
    let other = tempfile::tempdir().unwrap();
    symlink(
        dir.path().join("secrets.key"),
        other.path().join("secrets.key"),
    )
    .unwrap();
    assert!(crate::key::load(other.path()).is_err());
}
