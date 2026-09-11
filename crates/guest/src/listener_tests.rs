use super::*;
#[test]
fn only_host_vsock_peer_is_allowed() {
    assert!(is_host_peer(libc::AF_VSOCK as _, 2));
    assert!(!is_host_peer(libc::AF_VSOCK as _, 3));
    assert!(!is_host_peer(libc::AF_UNIX as _, 2));
}
