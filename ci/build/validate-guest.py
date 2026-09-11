#!/usr/bin/env python3
"""Reject guest executables that depend on a loader or shared libraries."""
import struct
import sys
from pathlib import Path


def validate(data):
    if len(data) < 64 or data[:7] != b'\x7fELF\x02\x01\x01':
        raise ValueError('expected ELF64 little-endian executable')
    kind, machine = struct.unpack_from('<HH', data, 16)
    if kind not in (2, 3) or machine != 62:
        raise ValueError('expected Linux x86_64 executable')
    offset = struct.unpack_from('<Q', data, 32)[0]
    size, count = struct.unpack_from('<HH', data, 54)
    if size != 56 or not 0 < count <= 1024 or offset + size * count > len(data):
        raise ValueError('invalid ELF program table')
    load = False
    for index in range(count):
        header = struct.unpack_from('<IIQQQQQQ', data, offset + size * index)
        load |= header[0] == 1
        if header[0] == 3:
            raise ValueError('guest must not have a dynamic interpreter')
        if header[0] == 2:
            start, length = header[2], header[5]
            if start + length > len(data) or length % 16:
                raise ValueError('invalid ELF dynamic section')
            if any(struct.unpack_from('<Q', data, entry)[0] == 1
                   for entry in range(start, start + length, 16)):
                raise ValueError('guest must not depend on shared libraries')
    if not load:
        raise ValueError('guest has no loadable segments')


if __name__ == '__main__':
    try:
        guest = Path(sys.argv[1]).read_bytes()
        validate(guest)
        if len(sys.argv) > 2 and guest not in Path(sys.argv[2]).read_bytes():
            raise ValueError('host does not embed the released guest executable')
    except (ValueError, OSError, IndexError) as error:
        sys.exit(f'Invalid guest binary: {error}')
