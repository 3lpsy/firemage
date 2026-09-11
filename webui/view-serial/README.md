Serial and terminal views shared by the VM sidebar and full page.

- Guest serial and Firecracker diagnostics use separate streams; historical combined logs remain explicitly labeled.
- Opt-in ttyS0 input requires an existing guest console program and administrator access.
- Locally bundled xterm.js renders byte-offset output; disconnect disposes polling and input handlers.
