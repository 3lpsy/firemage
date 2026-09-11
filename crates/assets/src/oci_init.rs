use anyhow::Context;

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
pub(crate) fn init(process: &serde_json::Value) -> anyhow::Result<String> {
    let args = process["args"]
        .as_array()
        .context("OCI image has no command")?;
    let mut init = String::from(
        r#"#!/bin/sh
set -eu
firemage_workload_mode=one-shot
firemage_stage=setup
finish() {
    status=$?
    trap - EXIT
    set +e
    mkdir -p /firemage/output
    printf '%s\n' "$status" > /firemage/output/exit-code
    printf '[firemage] %s exited with status %s\n' "$firemage_stage" "$status"
    sync
    if [ "$firemage_workload_mode" = keep-alive ]; then
        printf '[firemage] main program will not restart; guest remains running\n'
        while :; do sleep 3600; done
    fi
    printf '[firemage] stopping guest\n'
    reboot -f
    while :; do sleep 3600; done
}
trap finish EXIT
if [ ! -r /proc/mounts ]; then
    mount -t proc proc /proc
fi
ensure_mount() {
    while read -r device target rest; do
        if [ "$target" = "$2" ]; then return 0; fi
    done < /proc/mounts
    mount -t "$1" "$1" "$2"
}
ensure_mount sysfs /sys
ensure_mount devtmpfs /dev
mkdir -p /firemage/input /firemage/output
case " $(cat /proc/cmdline) " in
    *" firemage.seed=1 "*) mount -o ro /dev/vdb /firemage/input || exit 1;;
esac
"#,
    );
    if let Some(env) = process["env"].as_array() {
        for value in env {
            let value = value.as_str().context("invalid OCI environment")?;
            let (key, _) = value.split_once('=').context("invalid OCI environment")?;
            anyhow::ensure!(
                !key.is_empty()
                    && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                    && !key.as_bytes()[0].is_ascii_digit(),
                "invalid OCI environment key"
            );
            init += &format!("export {}\n", quote(value));
        }
    }
    init += "set --";
    for arg in args {
        init += &format!(" {}", quote(arg.as_str().context("invalid OCI argument")?));
    }
    init += "\nif [ -f /firemage/input/firemage/workload.sh ]; then\n    . /firemage/input/firemage/workload.sh\nfi\n";
    init += "if [ \"$#\" -eq 0 ] || [ -z \"$1\" ]; then\n    printf '[firemage] no workload command configured\\n' >&2\n    exit 127\nfi\n";
    init += "printf '[firemage] setup started\\n'\n";
    init += r#"if [ -f /firemage/input/firemage/network.sh ]; then
    /bin/sh /firemage/input/firemage/network.sh
fi
if [ -f /firemage/input/firemage/environment.sh ]; then
    . /firemage/input/firemage/environment.sh
fi
if [ -f /firemage/input/firemage/setup.sh ]; then
    /bin/sh /firemage/input/firemage/setup.sh
fi
if [ -f /firemage/input/user-data ]; then
    firemage_stage=userdata
    printf '[firemage] userdata started\n'
    /bin/sh /firemage/input/user-data
    printf '[firemage] userdata completed\n'
    firemage_stage=setup
fi
"#;
    init += &format!(
        "cd {} || exit 1\n",
        quote(process["cwd"].as_str().unwrap_or("/"))
    );
    init += "firemage_stage=workload\nprintf '[firemage] main program started\\n'\nset +e\n\"$@\"\nexit $?\n";
    Ok(init)
}

#[cfg(test)]
#[path = "oci_init_tests.rs"]
mod tests;
