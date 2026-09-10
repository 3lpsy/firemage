//! Desktop navigation names shared by the app shell.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Vms,
    Networks,
    Activity,
    Tokens,
    Secrets,
    Users,
    Config,
    Host,
}
impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Self::Vms => "Virtual machines",
            Self::Networks => "Networks",
            Self::Activity => "Activity",
            Self::Tokens => "API tokens",
            Self::Secrets => "Secrets",
            Self::Users => "Users",
            Self::Config => "Configuration",
            Self::Host => "Host",
        }
    }
    pub fn is_admin(self) -> bool {
        matches!(self, Self::Users | Self::Config | Self::Secrets)
    }
}

mod navigation;
pub use navigation::use_page;

impl Page {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Vms => "vms",
            Self::Networks => "networks",
            Self::Activity => "activity",
            Self::Tokens => "tokens",
            Self::Secrets => "secrets",
            Self::Users => "users",
            Self::Config => "config",
            Self::Host => "host",
        }
    }
    pub fn from_slug(slug: &str) -> Self {
        match slug {
            "networks" => Self::Networks,
            "activity" => Self::Activity,
            "tokens" => Self::Tokens,
            "secrets" => Self::Secrets,
            "users" => Self::Users,
            "config" => Self::Config,
            "host" => Self::Host,
            _ => Self::Vms,
        }
    }
}
