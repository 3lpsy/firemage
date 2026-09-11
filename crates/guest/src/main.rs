#[cfg(target_os = "linux")]
mod listener;
#[cfg(target_os = "linux")]
mod session;

fn main() {
    if let Err(error) = run() {
        eprintln!("firemage-guest: {error}");
        std::process::exit(1);
    }
}

fn run() -> std::io::Result<()> {
    let mut port = firemage_guest_protocol::PORT;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--version" | "-V" => {
                println!("firemage-guest {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--help" | "-h" => {
                println!(
                    "Usage: firemage-guest [--port PORT]\nRun the managed Web Shell guest service."
                );
                return Ok(());
            }
            "--port" => {
                port = args
                    .next()
                    .and_then(|value| value.parse().ok())
                    .filter(|value| *value > 0 && *value < u32::MAX)
                    .ok_or_else(|| std::io::Error::other("--port requires a valid vsock port"))?;
            }
            _ => return Err(std::io::Error::other("unknown argument; use --help")),
        }
    }
    #[cfg(target_os = "linux")]
    {
        listener::serve(port)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = port;
        Err(std::io::Error::other("guest service requires Linux"))
    }
}
