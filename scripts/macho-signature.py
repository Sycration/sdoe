#!/usr/bin/env python3
"""Checks the code signature page hashes of Mach-O files.

Usage: macho-signature.py FILE...

For every arm64 slice (thin or inside a universal file) the script requires
an LC_CODE_SIGNATURE whose CodeDirectory hashes match the file contents. An
arm64 Mac refuses to run code without a valid signature; an ad-hoc signature
from the linker is enough. Other slices are checked when they carry a
signature. Exit status 1 lists the files that fail.
"""

import hashlib
import struct
import sys

FAT_MAGIC = 0xCAFEBABE
FAT_MAGIC_64 = 0xCAFEBABF
MH_MAGIC_64 = 0xFEEDFACF
CPU_TYPE_ARM64 = 0x0100000C
LC_CODE_SIGNATURE = 0x1D
CSMAGIC_EMBEDDED_SIGNATURE = 0xFADE0CC0
CSMAGIC_CODEDIRECTORY = 0xFADE0C02
HASHES = {1: hashlib.sha1, 2: hashlib.sha256, 3: hashlib.sha256, 4: hashlib.sha384}


def slices(data):
    magic = struct.unpack_from(">I", data)[0]
    if magic in (FAT_MAGIC, FAT_MAGIC_64):
        n = struct.unpack_from(">I", data, 4)[0]
        for i in range(n):
            if magic == FAT_MAGIC:
                cpu, _, off, size, _ = struct.unpack_from(">iiIII", data, 8 + 20 * i)
            else:
                cpu, _, off, size, _, _ = struct.unpack_from(">iiQQII", data, 8 + 32 * i)
            yield cpu & 0xFFFFFFFF, data[off:off + size]
    elif struct.unpack_from("<I", data)[0] == MH_MAGIC_64:
        yield struct.unpack_from("<I", data, 4)[0], data
    else:
        raise ValueError("not a 64-bit Mach-O file")


def check_slice(m):
    """Returns None when the signature is valid, "unsigned" without one, or an error text."""
    ncmds = struct.unpack_from("<I", m, 16)[0]
    off = 32
    sig = None
    for _ in range(ncmds):
        cmd, size = struct.unpack_from("<II", m, off)
        if cmd == LC_CODE_SIGNATURE:
            sig = struct.unpack_from("<II", m, off + 8)
        off += size
    if sig is None:
        return "unsigned"
    dataoff, _ = sig
    magic, _, count = struct.unpack_from(">III", m, dataoff)
    if magic != CSMAGIC_EMBEDDED_SIGNATURE:
        return "bad signature blob"
    checked = 0
    for i in range(count):
        _, boff = struct.unpack_from(">II", m, dataoff + 12 + 8 * i)
        cd = dataoff + boff
        if struct.unpack_from(">I", m, cd)[0] != CSMAGIC_CODEDIRECTORY:
            continue
        (_, _, _, _, hash_off, _, _, ncode, code_limit, hash_size, hash_type, _, page_log) = \
            struct.unpack_from(">IIIIIIIIIBBBB", m, cd)
        page = 1 << page_log if page_log else code_limit
        h = HASHES.get(hash_type)
        if h is None:
            return f"unknown hash type {hash_type}"
        for slot in range(ncode):
            start = slot * page
            digest = h(m[start:min(start + page, code_limit)]).digest()[:hash_size]
            want = m[cd + hash_off + slot * hash_size: cd + hash_off + (slot + 1) * hash_size]
            if digest != want:
                return f"page {slot} does not match its hash"
        checked += 1
    return None if checked else "no code directory"


def main(paths):
    failed = []
    for path in paths:
        with open(path, "rb") as f:
            data = f.read()
        for cpu, m in slices(data):
            result = check_slice(m)
            arch = "arm64" if cpu == CPU_TYPE_ARM64 else hex(cpu)
            if result is None or (result == "unsigned" and cpu != CPU_TYPE_ARM64):
                continue
            failed.append(f"{path} ({arch}): {result}")
    for f in failed:
        print(f, file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
