use anyhow::Context;
use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    time::Instant,
};

pub(crate) struct Report {
    path: PathBuf,
    checks: Vec<Value>,
}
impl Report {
    pub(crate) fn new(directory: &Path) -> anyhow::Result<Self> {
        let report = Self {
            path: directory.join("infra-results.json"),
            checks: Vec::new(),
        };
        report.write("running")?;
        Ok(report)
    }

    pub(crate) fn run(&mut self, name: &str, command: &mut Command) -> anyhow::Result<()> {
        let directory = self.path.parent().expect("report directory");
        let stdout_name = format!("infra-{name}-stdout.log");
        let stderr_name = format!("infra-{name}-stderr.log");
        let started = Instant::now();
        let result = execute(
            command,
            &directory.join(&stdout_name),
            &directory.join(&stderr_name),
        );
        let succeeded = result.as_ref().is_ok_and(ExitStatus::success);
        self.checks.push(json!({
            "name":name, "status":if succeeded { "passed" } else { "failed" },
            "duration_ms": started.elapsed().as_millis(),
            "exit_code":result.as_ref().ok().and_then(ExitStatus::code),
            "error":result.as_ref().err().map(|error| format!("{error:#}")),
            "stdout":stdout_name, "stderr":stderr_name,
        }));
        self.write(if succeeded { "running" } else { "failed" })?;
        let status = result?;
        anyhow::ensure!(
            status.success(),
            "{name} infrastructure suite failed: {status}"
        );
        Ok(())
    }

    pub(crate) fn finish(&self) -> anyhow::Result<()> {
        self.write("passed")
    }

    fn write(&self, status: &str) -> anyhow::Result<()> {
        let output = json!({"schema":1,"status":status,"checks":self.checks});
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_vec_pretty(&output)?)?;
        std::fs::rename(temporary, &self.path)?;
        Ok(())
    }
}

fn execute(command: &mut Command, stdout: &Path, stderr: &Path) -> anyhow::Result<ExitStatus> {
    let stdout_log = File::create(stdout)?;
    let stderr_log = File::create(stderr)?;
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("cannot start embedded infrastructure harness")?;
    let stdout = child.stdout.take().expect("piped stdout");
    let stderr = child.stderr.take().expect("piped stderr");
    std::thread::scope(|scope| {
        let output = scope.spawn(|| copy(stdout, stdout_log, std::io::stdout()));
        let errors = scope.spawn(|| copy(stderr, stderr_log, std::io::stderr()));
        let status = child
            .wait()
            .context("cannot wait for infrastructure harness");
        output
            .join()
            .map_err(|_| anyhow::anyhow!("stdout capture failed"))??;
        errors
            .join()
            .map_err(|_| anyhow::anyhow!("stderr capture failed"))??;
        status
    })
}

fn copy(mut reader: impl Read, mut log: File, mut console: impl Write) -> std::io::Result<()> {
    let mut buffer = [0u8; 8192];
    loop {
        let size = reader.read(&mut buffer)?;
        if size == 0 {
            return Ok(());
        }
        log.write_all(&buffer[..size])?;
        // A closed console pipe must not discard the retained test evidence.
        let _ = console.write_all(&buffer[..size]);
        let _ = console.flush();
    }
}
