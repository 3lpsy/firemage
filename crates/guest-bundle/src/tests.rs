use crate::{bytes, validate};

#[test]
fn embedded_guest_is_static_and_truncated_files_are_rejected() {
    validate(bytes()).unwrap();
    for length in [0, 6, 63, 64] {
        assert!(validate(&bytes()[..length]).is_err());
    }
}

#[test]
fn reject_interpreters_and_shared_library_dependencies() {
    let mut elf = vec![0; 160];
    elf[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
    elf[16..18].copy_from_slice(&2_u16.to_le_bytes());
    elf[18..20].copy_from_slice(&62_u16.to_le_bytes());
    elf[32..40].copy_from_slice(&64_u64.to_le_bytes());
    elf[54..56].copy_from_slice(&56_u16.to_le_bytes());
    elf[56..58].copy_from_slice(&1_u16.to_le_bytes());
    elf[64..68].copy_from_slice(&3_u32.to_le_bytes());
    assert_eq!(validate(&elf), Err("dynamic interpreter is not supported"));
    elf[64..68].copy_from_slice(&2_u32.to_le_bytes());
    elf[72..80].copy_from_slice(&128_u64.to_le_bytes());
    elf[96..104].copy_from_slice(&16_u64.to_le_bytes());
    elf[128..136].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(validate(&elf), Err("shared libraries are not supported"));
}
