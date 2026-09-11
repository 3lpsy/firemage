use std::path::Path;

const SCRIPTS: &[(&str, &str)] = &[
    ("test-cli.py", include_str!("../scripts/test-cli.py")),
    ("test-vm.py", include_str!("../scripts/test-vm.py")),
    (
        "test-firecracker.py",
        include_str!("../scripts/test-firecracker.py"),
    ),
    ("vm/harness.py", include_str!("../scripts/vm/harness.py")),
    ("vm/cases.py", include_str!("../scripts/vm/cases.py")),
    (
        "vm/guest_files.py",
        include_str!("../scripts/vm/guest_files.py"),
    ),
    ("vm/oci.py", include_str!("../scripts/vm/oci.py")),
    ("vm/egress.py", include_str!("../scripts/vm/egress.py")),
    (
        "vm/egress_catalog.py",
        include_str!("../scripts/vm/egress_catalog.py"),
    ),
    (
        "vm/egress_recovery.py",
        include_str!("../scripts/vm/egress_recovery.py"),
    ),
    ("vm/fixture.py", include_str!("../scripts/vm/fixture.py")),
    ("vm/security.py", include_str!("../scripts/vm/security.py")),
    ("vm/init.sh", include_str!("../scripts/vm/init.sh")),
];

pub(crate) fn unpack(root: &Path) -> anyhow::Result<()> {
    let directory = root.join("crates/tests-infra/scripts");
    std::fs::create_dir_all(directory.join("vm"))?;
    for (name, content) in SCRIPTS {
        std::fs::write(directory.join(name), content)?;
    }
    Ok(())
}
