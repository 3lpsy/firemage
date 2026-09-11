use crate::Runtime;
use firemage_wire::{VmSpec, WorkloadMode};

impl Runtime {
    pub(crate) fn ensure_workload_init(&self, id: &str, spec: &VmSpec) -> anyhow::Result<()> {
        if spec
            .workload
            .as_ref()
            .is_some_and(|workload| workload.is_seed_init_required())
            && self.directory(id).join("rootfs.ext4").try_exists()?
        {
            anyhow::ensure!(
                std::fs::read(self.directory(id).join("oci-init-version"))
                    .is_ok_and(|version| version == b"1\n"),
                "this prepared OCI disk predates configurable workloads; create a new VM for a command override or keep-alive mode"
            );
        }
        Ok(())
    }
}

pub(crate) fn script(workload: &firemage_wire::Workload) -> String {
    let mode = match workload.mode {
        WorkloadMode::OneShot => "one-shot",
        WorkloadMode::KeepAlive => "keep-alive",
    };
    let mut script = format!("firemage_workload_mode='{mode}'\n");
    if let Some(command) = &workload.command {
        script += "set --";
        for argument in command {
            script += &format!(" '{}'", argument.replace('\'', "'\"'\"'"));
        }
        script += "\n";
    }
    script
}

#[cfg(test)]
#[path = "workload_tests.rs"]
mod tests;
