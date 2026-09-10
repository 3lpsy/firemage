use anyhow::Context;
use clap::{Args, ValueEnum};
use std::{
    io::{IsTerminal, Write},
    path::PathBuf,
    process::Command,
};

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum Suite {
    #[default]
    Verify,
    Cli,
    Firecracker,
    Vm,
}

#[derive(Args, Debug)]
pub struct Options {
    #[arg(long, value_enum, default_value_t = Suite::Verify)]
    pub suite: Suite,
    /// Explicitly authorize tests that create VMs and change host networking.
    #[arg(long)]
    pub confirm: bool,
    #[arg(
        long,
        env = "FIREMAGE_CI_RESULTS_DIR",
        default_value = "target/infra-results"
    )]
    pub results_dir: PathBuf,
    #[arg(
        long,
        env = "FIREMAGE_TEST_FIXTURES",
        default_value = "/opt/firemage/fixtures"
    )]
    pub fixtures_dir: PathBuf,
}

fn ensure_confirmed(confirm: bool) -> anyhow::Result<()> {
    if confirm {
        return Ok(());
    }
    anyhow::ensure!(
        std::io::stdin().is_terminal(),
        "infrastructure tests require --confirm without a terminal"
    );
    eprint!(
        "Run infrastructure tests on this host? Tests may start VMs and create TAP/firewall rules. Type 'yes': "
    );
    std::io::stderr().flush()?;
    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    anyhow::ensure!(answer.trim() == "yes", "infrastructure tests cancelled");
    Ok(())
}

pub fn run(options: Options) -> anyhow::Result<()> {
    ensure_confirmed(options.confirm)?;
    anyhow::ensure!(
        options.fixtures_dir.is_absolute(),
        "fixture directory must be absolute"
    );
    std::fs::create_dir_all(&options.results_dir)?;
    let results = std::fs::canonicalize(&options.results_dir)?;
    let binary = std::env::current_exe().context("cannot locate this development binary")?;
    let temporary = tempfile::Builder::new()
        .prefix("fm-infra-")
        .tempdir_in("/tmp")?;
    crate::assets::unpack(temporary.path())?;
    let cases: &[(&str, bool)] = match options.suite {
        Suite::Verify => &[
            ("test-cli.py", false),
            ("test-firecracker.py", true),
            ("test-vm.py", true),
        ],
        Suite::Cli => &[("test-cli.py", false)],
        Suite::Firecracker => &[("test-firecracker.py", true)],
        Suite::Vm => &[("test-vm.py", true)],
    };
    let mut report = crate::report::Report::new(&results)?;
    for (script, privileged) in cases {
        eprintln!("Running {script}");
        let mut command = if *privileged {
            let mut command = Command::new("sudo");
            command.args(["-n", "env"]);
            for (key, value) in [
                ("FIREMAGE_TEST_BINARY", binary.as_path()),
                ("FIREMAGE_CI_RESULTS_DIR", results.as_path()),
                ("FIREMAGE_TEST_FIXTURES", options.fixtures_dir.as_path()),
            ] {
                let mut variable = std::ffi::OsString::from(key);
                variable.push("=");
                variable.push(value);
                command.arg(variable);
            }
            command.arg("python3");
            command
        } else {
            Command::new("python3")
        };
        command
            .arg("-u")
            .arg(
                temporary
                    .path()
                    .join("crates/tests-infra/scripts")
                    .join(script),
            )
            .current_dir(temporary.path())
            .env("FIREMAGE_TEST_BINARY", &binary)
            .env("FIREMAGE_CI_RESULTS_DIR", &results)
            .env("FIREMAGE_TEST_FIXTURES", &options.fixtures_dir);
        let name = script
            .strip_prefix("test-")
            .and_then(|name| name.strip_suffix(".py"))
            .expect("suite script name");
        report.run(name, &mut command)?;
    }
    report.finish()?;
    eprintln!(
        "Infrastructure tests passed; results: {}",
        results.display()
    );
    Ok(())
}
