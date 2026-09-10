use firemage_egress_policy::{UpstreamProxy, ValueSource};

#[test]
fn upstream_credentials_require_valid_secret_references() {
    let mut proxy = UpstreamProxy {
        url: "http://proxy.example:3128".into(),
        username: Some(ValueSource::Literal("user".into())),
        password: Some(ValueSource::Literal("plaintext-secret".into())),
        ca_pem: None,
    };
    assert!(proxy.validate().is_err());
    proxy.password = Some(ValueSource::Secret {
        secret: "password".into(),
        prefix: String::new(),
    });
    assert!(proxy.validate().is_ok());
    proxy.username = Some(ValueSource::Secret {
        secret: "../../another-owner".into(),
        prefix: String::new(),
    });
    assert!(proxy.validate().is_err());
    proxy.username = Some(ValueSource::Literal("user".into()));
    proxy.password = Some(ValueSource::Secret {
        secret: "password".into(),
        prefix: "plaintext-prefix".into(),
    });
    assert!(proxy.validate().is_err());
    proxy.password = Some(ValueSource::Secret {
        secret: "password".into(),
        prefix: String::new(),
    });
    for url in [
        "http://user:password@proxy.example",
        "http://proxy.example:0",
        "http://proxy.example/path",
        "file:///tmp/proxy",
    ] {
        proxy.url = url.into();
        assert!(proxy.validate().is_err(), "{url}");
    }
}
