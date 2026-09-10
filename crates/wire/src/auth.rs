use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Login {
    pub username: String,
    pub password: String,
}
#[derive(Serialize, Deserialize)]
pub struct Token {
    pub token: String,
    pub expires_at: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateApiToken {
    pub name: String,
    pub expires_at: i64,
}
#[derive(Serialize, Deserialize)]
pub struct ApiToken {
    pub id: String,
    pub name: String,
    pub expires_at: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
    pub username: String,
    pub password: Option<String>,
    #[serde(default)]
    pub admin: bool,
    pub oidc_subject: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub oidc_subject: Option<String>,
    #[serde(default)]
    pub oidc_issuer: Option<String>,
    pub id: String,
    pub username: String,
    pub admin: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpdateUser {
    pub username: String,
    pub admin: bool,
    pub disabled: bool,
    pub password: Option<String>,
    pub oidc_subject: Option<String>,
    #[serde(default)]
    pub clear_oidc: bool,
}
