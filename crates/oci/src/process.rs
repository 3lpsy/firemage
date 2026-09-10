use anyhow::ensure;
use oci_spec::image::ImageConfiguration;
use serde_json::{Value, json};

pub(crate) fn process(image: &ImageConfiguration) -> anyhow::Result<Value> {
    let config = image
        .config()
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("OCI image has no command configuration"))?;
    let user = config.user().as_deref().unwrap_or("");
    ensure!(
        matches!(
            user,
            "" | "0" | "root" | "0:0" | "root:root" | "root:0" | "0:root"
        ),
        "OCI non-root USER requires a BYO init"
    );
    let args: Vec<_> = config
        .entrypoint()
        .iter()
        .flatten()
        .chain(config.cmd().iter().flatten())
        .cloned()
        .collect();
    ensure!(
        !args.is_empty() && !args[0].is_empty(),
        "OCI image has no command"
    );
    ensure!(
        args.iter().all(|arg| !arg.contains('\0')),
        "OCI command contains NUL"
    );
    let env = config.env().clone().unwrap_or_default();
    for value in &env {
        let (key, _) = value
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("invalid OCI environment"))?;
        ensure!(
            !key.is_empty()
                && !key.as_bytes()[0].is_ascii_digit()
                && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                && !value.contains('\0'),
            "invalid OCI environment"
        );
    }
    let cwd = config
        .working_dir()
        .as_deref()
        .filter(|v| !v.is_empty())
        .unwrap_or("/");
    ensure!(
        cwd.starts_with('/') && !cwd.contains('\0'),
        "OCI working directory must be absolute"
    );
    Ok(json!({"args":args,"env":env,"cwd":cwd,"user":{"uid":0,"gid":0}}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn combines_entrypoint_and_arguments_without_shell_interpretation() {
        let config: ImageConfiguration = serde_json::from_value(json!({
            "architecture":"amd64","os":"linux","rootfs":{"type":"layers","diff_ids":[]},
            "config":{"Entrypoint":["/bin/sh","-c"],"Cmd":["echo 'hello'"],"WorkingDir":"","User":"root"}
        })).unwrap();
        let process = process(&config).unwrap();
        assert_eq!(process["args"], json!(["/bin/sh", "-c", "echo 'hello'"]));
        assert_eq!(process["cwd"], "/");
    }
}
