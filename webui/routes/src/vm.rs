#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum VmTab {
    #[default]
    Overview,
    Configuration,
    Isolation,
    Egress,
    Environment,
    Attachments,
    Boot,
    Serial,
    WebShell,
    Files,
    Snapshots,
    Metadata,
    FirecrackerLogs,
    Advanced,
}
impl VmTab {
    pub const ALL: [Self; 14] = [
        Self::Overview,
        Self::Configuration,
        Self::Isolation,
        Self::Egress,
        Self::Environment,
        Self::Attachments,
        Self::Boot,
        Self::Serial,
        Self::WebShell,
        Self::Files,
        Self::Snapshots,
        Self::Metadata,
        Self::FirecrackerLogs,
        Self::Advanced,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Configuration => "Configuration",
            Self::Isolation => "Isolation",
            Self::Egress => "Egress",
            Self::Environment => "Environment",
            Self::Attachments => "Attachments",
            Self::Boot => "Boot",
            Self::Serial => "Serial",
            Self::WebShell => "Web Shell",
            Self::Files => "Files",
            Self::Snapshots => "Snapshots",
            Self::Metadata => "Metadata",
            Self::FirecrackerLogs => "Firecracker Logs",
            Self::Advanced => "Advanced",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Configuration => "configuration",
            Self::Isolation => "isolation",
            Self::Egress => "egress",
            Self::Environment => "environment",
            Self::Attachments => "attachments",
            Self::Boot => "boot",
            Self::Serial => "serial",
            Self::WebShell => "web-shell",
            Self::Files => "files",
            Self::Snapshots => "snapshots",
            Self::Metadata => "metadata",
            Self::FirecrackerLogs => "firecracker-logs",
            Self::Advanced => "advanced",
        }
    }
    pub fn from_slug(slug: &str) -> Self {
        if slug == "security" {
            return Self::Isolation;
        }
        Self::ALL
            .into_iter()
            .find(|tab| tab.slug() == slug)
            .unwrap_or_default()
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum VmPath {
    Inventory,
    Create,
    Edit(String),
    Detail { id: String, tab: VmTab },
}
pub fn parse_vm_path(path: &str) -> VmPath {
    if path.is_empty() {
        return VmPath::Inventory;
    }
    if path == "new" {
        return VmPath::Create;
    }
    let (id, suffix) = path.split_once('/').unwrap_or((path, ""));
    if id.is_empty() || suffix.contains('/') {
        return VmPath::Inventory;
    }
    if suffix == "edit" {
        return VmPath::Edit(id.into());
    }
    VmPath::Detail {
        id: id.into(),
        tab: VmTab::from_slug(suffix),
    }
}

#[cfg(test)]
#[path = "vm_tests.rs"]
mod tests;
