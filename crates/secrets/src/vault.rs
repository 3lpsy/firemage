use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use firemage_queries::DatabaseConnection;
use firemage_wire::SecretMetadata;
use rand::RngCore;
use std::{path::Path, sync::Arc};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Vault {
    db: DatabaseConnection,
    key: Arc<Zeroizing<[u8; 32]>>,
}
impl Vault {
    pub async fn new(db: DatabaseConnection, data_dir: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            db,
            key: Arc::new(crate::key::load(data_dir)?),
        })
    }
    pub async fn put(
        &self,
        owner: &str,
        name: &str,
        value: &str,
    ) -> anyhow::Result<SecretMetadata> {
        firemage_wire::ensure_name(name)?;
        anyhow::ensure!(
            !value.is_empty() && value.len() <= 65536 && !value.contains('\0'),
            "secret must contain 1-65536 bytes without NUL"
        );
        let mut nonce = [0u8; 24];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let aad = format!("firemage-secret-v1\0{owner}\0{name}");
        let ciphertext = self
            .cipher()
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: value.as_bytes(),
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| anyhow::anyhow!("secret encryption failed"))?;
        let encoded = format!("v1:{}:{}", hex::encode(nonce), hex::encode(ciphertext));
        let updated_at = firemage_queries::put_secret(&self.db, owner, name, encoded).await?;
        Ok(SecretMetadata {
            name: name.into(),
            updated_at,
        })
    }
    pub async fn list(&self, owner: &str) -> anyhow::Result<Vec<SecretMetadata>> {
        Ok(firemage_queries::secrets(&self.db, owner)
            .await?
            .into_iter()
            .map(|row| SecretMetadata {
                name: row.name,
                updated_at: row.updated_at,
            })
            .collect())
    }
    pub async fn delete(&self, owner: &str, name: &str) -> anyhow::Result<bool> {
        firemage_wire::ensure_name(name)?;
        firemage_queries::delete_secret(&self.db, owner, name).await
    }
    pub async fn resolve(&self, owner: &str, name: &str) -> anyhow::Result<String> {
        firemage_wire::ensure_name(name)?;
        let row = firemage_queries::secret(&self.db, owner, name).await?;
        let fail = || anyhow::anyhow!("secret decryption failed");
        let parts: Vec<_> = row.ciphertext.split(':').collect();
        anyhow::ensure!(
            parts.len() == 3 && parts[0] == "v1",
            "secret decryption failed"
        );
        let nonce = hex::decode(parts[1]).map_err(|_| fail())?;
        anyhow::ensure!(nonce.len() == 24, "secret decryption failed");
        let ciphertext = hex::decode(parts[2]).map_err(|_| fail())?;
        let aad = format!("firemage-secret-v1\0{owner}\0{name}");
        let plain = self
            .cipher()
            .decrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: &ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| fail())?;
        String::from_utf8(plain).map_err(|_| fail())
    }
    fn cipher(&self) -> XChaCha20Poly1305 {
        XChaCha20Poly1305::new_from_slice(self.key.as_ref().as_ref()).expect("32-byte key")
    }
    pub fn scoped(&self, owner: String) -> ScopedSecrets {
        ScopedSecrets {
            vault: self.clone(),
            owner,
        }
    }
}
pub struct ScopedSecrets {
    vault: Vault,
    owner: String,
}
#[async_trait::async_trait]
impl firemage_request_signing::SecretResolver for ScopedSecrets {
    async fn resolve(&self, name: &str) -> anyhow::Result<String> {
        self.vault.resolve(&self.owner, name).await
    }
}
