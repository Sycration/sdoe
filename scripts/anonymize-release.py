#!/usr/bin/env python3
"""Removes build-machine details from release assets and checks the result.

  anonymize-release.py normalize FILE...
      Sets every timestamp to the Unix epoch: zip entries (the DOS date field
      gets its minimum, 1980-01-01 00:00, and an extended timestamp field
      holds 0), the PE headers of Windows executables and DLLs (also inside
      zip files), and the modification time of FILE itself. Zip entries keep
      their order, contents and permissions; owner ids are not stored.

  anonymize-release.py check [--forbid TEXT]... [--word TEXT]... [--allow TEXT]... FILE...
      Fails when a file contains a --forbid text (anywhere) or a --word text
      (as a whole word, case-sensitive), or a timestamp other than the epoch.
      Zip, tar.gz and AppImage contents and gzip streams embedded in files
      are checked too. --allow texts are removed before the search (public
      identifiers that contain a forbidden text).
"""

import gzip
import io
import os
import re
import struct
import subprocess
import sys
import tarfile
import tempfile
import zipfile
import zlib

DOS_EPOCH = (1980, 1, 1, 0, 0, 0)
# Extended timestamp extra field (0x5455): flags 1 (modification time), time 0.
UT_EPOCH = b"UT" + struct.pack("<HBI", 5, 1, 0)


# ---------------------------------------------------------------------------
# PE (Windows executables and DLLs)
# ---------------------------------------------------------------------------

def pe_timestamp_offsets(b):
    """File offsets of the 32-bit timestamps of a PE file: COFF header,
    export directory, resource directories and debug directory entries."""
    if len(b) < 0x40 or b[:2] != b"MZ":
        return []
    pe = struct.unpack_from("<I", b, 0x3C)[0]
    if pe + 24 > len(b) or b[pe:pe + 4] != b"PE\0\0":
        return []
    nsec = struct.unpack_from("<H", b, pe + 6)[0]
    opt_size = struct.unpack_from("<H", b, pe + 20)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", b, opt)[0]
    dirs = opt + (112 if magic == 0x20B else 96)
    ndirs = struct.unpack_from("<I", b, dirs - 4)[0]
    sections = []
    for i in range(nsec):
        s = opt + opt_size + 40 * i
        vsize, va, rsize, rptr = struct.unpack_from("<IIII", b, s + 8)
        sections.append((va, max(vsize, rsize), rptr))

    def off(rva):
        for va, size, ptr in sections:
            if va <= rva < va + size:
                return ptr + rva - va
        return None

    def directory(i):
        if i >= ndirs:
            return None, 0
        rva, size = struct.unpack_from("<II", b, dirs + 8 * i)
        return (off(rva) if rva else None), size

    out = [pe + 8]
    exp, _ = directory(0)
    if exp is not None:
        out.append(exp + 4)
    res, _ = directory(2)
    if res is not None:
        todo, seen = [res], set()
        while todo:
            d = todo.pop()
            if d in seen or d + 16 > len(b):
                continue
            seen.add(d)
            out.append(d + 4)
            n = sum(struct.unpack_from("<HH", b, d + 12))
            for k in range(n):
                entry = struct.unpack_from("<I", b, d + 16 + 8 * k + 4)[0]
                if entry & 0x80000000:
                    todo.append(res + (entry & 0x7FFFFFFF))
    dbg, size = directory(6)
    if dbg is not None:
        out.extend(dbg + 28 * k + 4 for k in range(size // 28))
    return [o for o in out if o + 4 <= len(b)]


def pe_checksum(b, field):
    total = 0
    for i in range(0, len(b) - len(b) % 2, 2):
        if field <= i < field + 4:
            continue
        total += b[i] | (b[i + 1] << 8)
        total = (total & 0xFFFF) + (total >> 16)
    if len(b) % 2:
        total += b[-1]
        total = (total & 0xFFFF) + (total >> 16)
    total = (total & 0xFFFF) + (total >> 16)
    return (total + len(b)) & 0xFFFFFFFF


def pe_normalize(b):
    """Returns `b` with every PE timestamp set to 0 (and the checksum
    recomputed when the file has one), or `b` unchanged for other files."""
    offsets = pe_timestamp_offsets(b)
    if not offsets:
        return b
    b = bytearray(b)
    for o in offsets:
        b[o:o + 4] = b"\0\0\0\0"
    pe = struct.unpack_from("<I", b, 0x3C)[0]
    field = pe + 24 + 64
    if struct.unpack_from("<I", b, field)[0]:
        struct.pack_into("<I", b, field, pe_checksum(b, field))
    return bytes(b)


def pe_timestamps(b):
    return [struct.unpack_from("<I", b, o)[0] for o in pe_timestamp_offsets(b)]


# ---------------------------------------------------------------------------
# Normalize
# ---------------------------------------------------------------------------

def normalize_zip(path):
    buf = io.BytesIO()
    with zipfile.ZipFile(path) as src, zipfile.ZipFile(buf, "w") as dst:
        for info in src.infolist():
            data = src.read(info)
            if not info.is_dir():
                data = pe_normalize(data)
            new = zipfile.ZipInfo(info.filename, DOS_EPOCH)
            new.create_system = 3
            new.external_attr = info.external_attr
            new.extra = UT_EPOCH
            dirlike = info.is_dir() or (info.external_attr >> 16) & 0o170000 == 0o120000
            new.compress_type = zipfile.ZIP_STORED if dirlike else zipfile.ZIP_DEFLATED
            dst.writestr(new, data, compresslevel=9)
    with open(path, "wb") as f:
        f.write(buf.getvalue())


def normalize(paths):
    for p in paths:
        if p.endswith(".zip"):
            normalize_zip(p)
        else:
            with open(p, "rb") as f:
                b = f.read()
            n = pe_normalize(b)
            if n != b:
                with open(p, "wb") as f:
                    f.write(n)
        os.utime(p, (0, 0), follow_symlinks=False)


# ---------------------------------------------------------------------------
# Check
# ---------------------------------------------------------------------------

class Checker:
    def __init__(self, forbid, words, allow):
        self.forbid = [t.encode() for t in forbid if t]
        self.words = [re.compile(rb"(?<![A-Za-z0-9_])" + re.escape(w.encode()) + rb"(?![A-Za-z0-9_])") for w in words if w]
        self.allow = [a.encode() for a in allow if a]
        self.problems = []

    def problem(self, where, what):
        self.problems.append(f"{where}: {what}")

    def scan_bytes(self, where, b, depth=0):
        for a in self.allow:
            b = b.replace(a, b"")
        for t in self.forbid:
            i = b.find(t)
            if i >= 0:
                ctx = re.sub(rb"[^\x20-\x7e]", b".", b[max(0, i - 40):i + len(t) + 40]).decode()
                self.problem(where, f"contains {t.decode()!r}: ...{ctx}...")
        for w in self.words:
            m = w.search(b)
            if m:
                ctx = re.sub(rb"[^\x20-\x7e]", b".", b[max(0, m.start() - 40):m.end() + 40]).decode()
                self.problem(where, f"contains the word {m.group().decode()!r}: ...{ctx}...")
        stamps = [t for t in pe_timestamps(b) if t]
        if stamps:
            self.problem(where, f"PE timestamps {stamps}")
        if depth == 0:
            # Embedded gzip streams, such as the web assets of sdoe-web.
            for m in re.finditer(rb"\x1f\x8b\x08", b):
                try:
                    d = zlib.decompressobj(31)
                    data = d.decompress(b[m.start():], 1 << 30)
                except zlib.error:
                    continue
                if len(data) > 64:
                    self.scan_bytes(f"{where} (gzip stream at {m.start()})", data, depth + 1)

    def check_zip(self, path, name):
        with zipfile.ZipFile(path) as z:
            for info in z.infolist():
                where = f"{name}/{info.filename}"
                if info.date_time != DOS_EPOCH:
                    self.problem(where, f"zip date {info.date_time}")
                if UT_EPOCH not in info.extra:
                    self.problem(where, "zip entry without an epoch extended timestamp")
                if not info.is_dir():
                    self.scan_bytes(where, z.read(info))

    def check_tar(self, path, name):
        with open(path, "rb") as f:
            head = f.read(10)
        if struct.unpack_from("<I", head, 4)[0]:
            self.problem(name, "gzip header time is not 0")
        with tarfile.open(path) as t:
            for m in t.getmembers():
                where = f"{name}/{m.name}"
                if m.mtime or m.uid or m.gid or m.uname or m.gname:
                    self.problem(where, f"tar metadata mtime={m.mtime} uid={m.uid} gid={m.gid} uname={m.uname!r} gname={m.gname!r}")
                if m.isfile():
                    self.scan_bytes(where, t.extractfile(m).read())

    def check_appimage(self, path, name):
        with open(path, "rb") as f:
            self.scan_bytes(name, f.read())
        offset = subprocess.run([path, "--appimage-offset"], capture_output=True, text=True,
                                env={**os.environ, "APPIMAGE_EXTRACT_AND_RUN": "1"}).stdout.strip()
        if not offset.isdigit():
            self.problem(name, "no squashfs offset")
            return
        env = {**os.environ, "TZ": "UTC"}
        info = subprocess.run(["unsquashfs", "-s", "-o", offset, path], capture_output=True, text=True, env=env).stdout
        m = re.search(r"Creation or last append time (.*)", info)
        if not m or "1970" not in m.group(1):
            self.problem(name, f"squashfs creation time {m.group(1) if m else '?'}")
        listing = subprocess.run(["unsquashfs", "-lln", "-UTC", "-o", offset, path], capture_output=True, text=True, env=env).stdout
        for line in listing.splitlines():
            f = line.split()
            if len(f) >= 6 and re.match(r"\d{4}-\d\d-\d\d", f[3]):
                if f[3] != "1970-01-01" or f[4] != "00:00":
                    self.problem(name, f"squashfs entry time: {line}")
                    break
                if f[1] != "0/0":
                    self.problem(name, f"squashfs entry owner: {line}")
                    break
        with tempfile.TemporaryDirectory() as d:
            root = os.path.join(d, "root")
            # Exit status 2: files extracted, some attributes not set.
            r = subprocess.run(["unsquashfs", "-q", "-no-xattrs", "-d", root, "-o", offset, path], capture_output=True, text=True)
            if r.returncode not in (0, 2):
                self.problem(name, f"unsquashfs failed: {r.stderr.strip()}")
            for dp, _, fn in os.walk(root):
                for f in fn:
                    p = os.path.join(dp, f)
                    if not os.path.islink(p):
                        with open(p, "rb") as h:
                            self.scan_bytes(f"{name}/{os.path.relpath(p, root)}", h.read())

    def check(self, path):
        name = os.path.basename(path)
        if os.stat(path, follow_symlinks=False).st_mtime != 0:
            self.problem(name, "file modification time is not the epoch")
        if name.endswith(".zip"):
            self.check_zip(path, name)
        elif name.endswith(".tar.gz"):
            self.check_tar(path, name)
        elif name.endswith(".AppImage"):
            self.check_appimage(path, name)
        else:
            with open(path, "rb") as f:
                self.scan_bytes(name, f.read())


def main(argv):
    if len(argv) < 2 or argv[1] not in ("normalize", "check"):
        sys.exit(__doc__)
    if argv[1] == "normalize":
        normalize(argv[2:])
        return
    forbid, words, allow, files = [], [], [], []
    args = iter(argv[2:])
    for a in args:
        if a == "--forbid":
            forbid.append(next(args))
        elif a == "--word":
            words.append(next(args))
        elif a == "--allow":
            allow.append(next(args))
        else:
            files.append(a)
    c = Checker(forbid, words, allow)
    for f in files:
        c.check(f)
    for p in c.problems:
        print(p, file=sys.stderr)
    if c.problems:
        sys.exit(f"{len(c.problems)} problems in the release assets")
    print(f"checked {len(files)} assets: no build-machine details, all timestamps at the epoch")


if __name__ == "__main__":
    main(sys.argv)
