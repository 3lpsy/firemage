Serial views shared by the VM sidebar and full page.

- Guest serial and Firecracker diagnostics use separate streams; historical combined logs remain explicitly labeled.
- Tabs, refresh, connection status, and reconnect actions share one toolbar.
- Opening TTY Stream connects automatically. Disconnect and failures retain output; reconnect starts fresh polling and discards pending input.
- Opt-in ttyS0 input requires an existing guest console program and administrator access. Configure it in the VM form.
- Locally bundled xterm.js renders byte-offset output; leaving the stream disposes polling and input handlers.
