Optional Web Shell view shared by the VM inspector and full page.

- `Settings` supplies the create/edit form's switch and shell argv input.
- `WebShell` enables stopped VMs and opens an administrator browser session.
- The xterm client uses single-use WebSocket tickets, bounded queues, resize, and reconnect.
- Leaving the view closes its shell; ttyS0 remains in the independent Serial view.
