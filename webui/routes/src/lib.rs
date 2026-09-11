//! Desktop navigation names shared by the app shell.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Vms,
    Snapshots,
    Networks,
    Egress,
    Kernels,
    Assets,
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
            Self::Snapshots => "Snapshots",
            Self::Networks => "Networks",
            Self::Egress => "Egress",
            Self::Kernels => "Kernels",
            Self::Assets => "Assets",
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
pub use navigation::{
    navigate, navigate_vm, navigate_vm_editor, use_egress_path, use_page, use_vm_id,
};

impl Page {
    pub fn slug(self) -> &'static str {
        match self {
            Self::Vms => "vms",
            Self::Snapshots => "snapshots",
            Self::Networks => "networks",
            Self::Egress => "egress",
            Self::Kernels => "kernels",
            Self::Assets => "assets",
            Self::Activity => "activity",
            Self::Tokens => "tokens",
            Self::Secrets => "secrets",
            Self::Users => "users",
            Self::Config => "config",
            Self::Host => "host",
        }
    }
    pub fn from_slug(slug: &str) -> Self {
        match slug.split('/').next().unwrap_or_default() {
            "snapshots" => Self::Snapshots,
            "networks" => Self::Networks,
            "egress" => Self::Egress,
            "kernels" => Self::Kernels,
            "assets" => Self::Assets,
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
