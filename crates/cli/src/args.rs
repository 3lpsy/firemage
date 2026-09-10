use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "firemage",
    version,
    about = "Firecracker VM management over HTTPS and Unix sockets"
)]
pub struct Cli {
    #[arg(short, long, env = "FIREMAGE_CONFIG", global = true)]
    pub config: Option<PathBuf>,
    #[command(flatten)]
    pub client: firemage_config::Client,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Subcommand)]
pub enum Command {
    Serve(Box<firemage_config::Server>),
    /// Run embedded infrastructure acceptance tests on a disposable host.
    #[cfg(feature = "tests-infra")]
    TestsInfra(firemage_tests_infra::Options),
    Login(Login),
    /// Print only a new session token. Does not save credentials or config.
    Authtoken(Login),
    #[command(subcommand)]
    Secret(crate::secret::Command),
    /// Manage the server's kernel catalog.
    #[command(subcommand)]
    Kernel(crate::kernel::Command),
    /// Manage reusable VM input files owned by your account.
    #[command(subcommand)]
    Asset(crate::asset::Command),
    Logout,
    Whoami,
    #[command(subcommand)]
    Apitoken(ApiToken),
    #[command(subcommand)]
    User(User),
    #[command(subcommand)]
    Vm(Vm),
    #[command(subcommand)]
    Network(Network),
}
#[derive(Args)]
pub struct Login {
    #[arg(long)]
    pub refresh: bool,
}
#[derive(Subcommand)]
pub enum ApiToken {
    List,
    Create {
        name: String,
        #[arg(long, help = "Expiration as Unix timestamp in seconds")]
        expires_at: i64,
    },
    Delete {
        id: String,
    },
}
#[derive(Subcommand)]
pub enum User {
    /// Create the first administrator locally, before starting the server.
    Bootstrap {
        username: String,
        #[command(flatten)]
        server: Box<firemage_config::Server>,
    },
    Create {
        username: String,
        #[arg(long)]
        admin: bool,
        #[arg(long)]
        oidc_subject: Option<String>,
    },
    List,
}
#[derive(Subcommand)]
pub enum Vm {
    List,
    Create {
        file: PathBuf,
        #[arg(long)]
        input: Vec<String>,
    },
    Run {
        file: PathBuf,
        #[arg(long)]
        input: Vec<String>,
        #[arg(long)]
        wait: bool,
        #[arg(long, default_value_t = 900)]
        timeout: u64,
    },
    Get {
        id: String,
    },
    /// Replace a stopped VM definition from TOML.
    Update {
        id: String,
        file: PathBuf,
        #[arg(long)]
        input: Vec<String>,
    },
    /// Inspect effective proxy and tunnel routes.
    Egress {
        id: String,
    },
    Delete {
        id: String,
    },
    Launch {
        id: String,
    },
    Prepare {
        id: String,
    },
    Start {
        id: String,
    },
    Pause {
        id: String,
    },
    Resume {
        id: String,
    },
    Shutdown {
        id: String,
    },
    Stop {
        id: String,
    },
    Refresh {
        id: String,
    },
    Wait {
        id: String,
        #[arg(long, default_value_t = 900)]
        timeout: u64,
    },
    Snapshot {
        id: String,
        #[arg(long)]
        snapshot_path: String,
        #[arg(long)]
        memory_path: String,
    },
    Restore {
        id: String,
        #[arg(long)]
        snapshot_path: String,
        #[arg(long)]
        memory_path: String,
    },
    Metadata {
        id: String,
        file: PathBuf,
    },
    /// Forward any Firecracker API operation. Body file uses TOML.
    Api {
        id: String,
        method: String,
        path: String,
        #[arg(long)]
        file: Option<PathBuf>,
    },
    /// Copy a file out of a stopped guest root filesystem.
    Cp {
        id: String,
        guest_path: String,
        output: PathBuf,
    },
    Logs {
        id: String,
    },
}
#[derive(Subcommand)]
pub enum Network {
    List,
    Create { file: PathBuf },
    Delete { name: String },
}
