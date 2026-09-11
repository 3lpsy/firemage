/// Validate a static Linux x86_64 ELF executable without running it.
pub fn validate(bytes: &[u8]) -> Result<(), &'static str> {
    let read = |offset: usize, count: usize| bytes.get(offset..offset.checked_add(count)?);
    let u16_at = |offset| read(offset, 2).map(|v| u16::from_le_bytes(v.try_into().unwrap()));
    let u64_at = |offset| read(offset, 8).map(|v| u64::from_le_bytes(v.try_into().unwrap()));
    if read(0, 7) != Some(b"\x7fELF\x02\x01\x01".as_slice())
        || !matches!(u16_at(16), Some(2 | 3))
        || u16_at(18) != Some(62)
    {
        return Err("unsupported ELF header");
    }
    let offset = usize::try_from(u64_at(32).ok_or("missing program table")?)
        .map_err(|_| "invalid program table")?;
    let size = usize::from(u16_at(54).ok_or("missing program size")?);
    let count = usize::from(u16_at(56).ok_or("missing program count")?);
    if size != 56 || count == 0 || count > 1024 {
        return Err("invalid program table");
    }
    let mut load = false;
    for index in 0..count {
        let header = read(offset.checked_add(index * size).ok_or("overflow")?, size)
            .ok_or("truncated program table")?;
        let kind = u32::from_le_bytes(header[..4].try_into().unwrap());
        if kind == 3 {
            return Err("dynamic interpreter is not supported");
        }
        load |= kind == 1;
        if kind == 2 {
            let offset = usize::try_from(u64::from_le_bytes(header[8..16].try_into().unwrap()))
                .map_err(|_| "invalid dynamic section")?;
            let length = usize::try_from(u64::from_le_bytes(header[32..40].try_into().unwrap()))
                .map_err(|_| "invalid dynamic section")?;
            if length % 16 != 0 {
                return Err("invalid dynamic section size");
            }
            for entry in read(offset, length)
                .ok_or("truncated dynamic section")?
                .as_chunks::<16>()
                .0
            {
                if u64::from_le_bytes(entry[..8].try_into().unwrap()) == 1 {
                    return Err("shared libraries are not supported");
                }
            }
        }
    }
    if !load {
        return Err("missing executable segments");
    }
    Ok(())
}
