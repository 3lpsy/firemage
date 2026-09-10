use super::*;
use crate::{SigningConfig, ValueSource};
use std::collections::BTreeMap;
struct Secrets;
#[async_trait::async_trait]
impl SecretResolver for Secrets {
    async fn resolve(&self, name: &str) -> anyhow::Result<String> {
        match name {
            "key" => Ok("test-key".into()),
            "aws" => Ok("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".into()),
            _ => anyhow::bail!("sensitive upstream failure"),
        }
    }
}
#[test]
fn rfc4231_hmac_sha256_vector() {
    assert_eq!(
        hex::encode(super::apply::hmac(&[0x0b; 20], b"Hi There")),
        "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
    );
}
#[tokio::test]
async fn injection_replaces_headers_and_masks_secret_errors() {
    let mut headers = http::HeaderMap::new();
    headers.append("authorization", "guest".parse().unwrap());
    headers.append("authorization", "duplicate".parse().unwrap());
    let injections = BTreeMap::from([(
        "Authorization".into(),
        ValueSource::Secret {
            secret: "key".into(),
            prefix: "Bearer ".into(),
        },
    )]);
    apply(
        &Secrets,
        &injections,
        None,
        "GET",
        "https://example.com/",
        &mut headers,
        b"",
    )
    .await
    .unwrap();
    assert_eq!(headers.get_all("authorization").iter().count(), 1);
    assert_eq!(headers["authorization"], "Bearer test-key");
    assert!(headers["authorization"].is_sensitive());
    let error = resolve(
        &ValueSource::Secret {
            secret: "absent".into(),
            prefix: String::new(),
        },
        &Secrets,
    )
    .await
    .unwrap_err();
    assert!(!error.to_string().contains("sensitive upstream"));
}
#[tokio::test]
async fn s3_published_get_object_signature() {
    let config = SigningConfig::AwsSigv4 {
        region: "us-east-1".into(),
        service: "s3".into(),
        access_key: ValueSource::Literal("AKIAIOSFODNN7EXAMPLE".into()),
        secret_key: ValueSource::Secret {
            secret: "aws".into(),
            prefix: String::new(),
        },
        session_token: None,
    };
    let mut headers = http::HeaderMap::new();
    headers.insert("range", "bytes=0-9".parse().unwrap());
    super::aws::apply(
        &Secrets,
        &config,
        "GET",
        "https://examplebucket.s3.amazonaws.com/test.txt",
        &mut headers,
        b"",
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(1369353600),
    )
    .await
    .unwrap();
    assert_eq!(headers["x-amz-date"], "20130524T000000Z");
    assert!(
        headers["authorization"].to_str().unwrap().ends_with(
            "Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        )
    );
}
#[test]
fn invalid_policy_cannot_override_transport_or_embed_signing_keys() {
    assert!(
        crate::validate(
            &BTreeMap::from([("Host".into(), ValueSource::Literal("attacker".into()))]),
            None
        )
        .is_err()
    );
    let config = SigningConfig::HmacSha256 {
        key: ValueSource::Literal("plaintext".into()),
        header: "x-signature".into(),
        prefix: String::new(),
    };
    assert!(crate::validate(&BTreeMap::new(), Some(&config)).is_err());
}
