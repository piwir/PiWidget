#!/usr/bin/env python3
"""Verify that a Windows executable really embeds icon resources.

GPUI's Windows backend loads the window / taskbar icon with
    LoadImageW(GetModuleHandleW(None), PCWSTR(1), IMAGE_ICON, ...)
so the PE must contain:
  - RT_ICON       (type 3)  one entry per size
  - RT_GROUP_ICON (type 14) with resource ID 1

Those directories only exist when build.rs compiled piwidget.rc and the linker
pulled it in. CI runs this before packaging so we never ship an iconless build.

All output is ASCII on purpose: Windows consoles / CI runners default to a
legacy codepage (cp1252), and printing non-ASCII there raises
UnicodeEncodeError before the check result can be read.

Usage: python scripts/check_windows_icon.py target/release/piwidget.exe
"""

import struct
import sys

# Windows consoles and CI log collectors default to a legacy codepage (cp1252
# and friends); printing non-ASCII there raises UnicodeEncodeError instead of
# logging. Try UTF-8 first, and fall back to errors="replace" so the check
# result is never lost to an encoding error.
for _stream in (sys.stdout, sys.stderr):
    try:
        _stream.reconfigure(encoding="utf-8", errors="replace")
    except (AttributeError, ValueError, OSError):
        pass

RT_ICON = 3
RT_GROUP_ICON = 14
IMAGE_DIRECTORY_ENTRY_RESOURCE = 2


def read_pe(data):
    """Return (resource RVA, resource size, section table). PE32 / PE32+ only."""
    if data[:2] != b"MZ":
        raise ValueError("not a PE file (missing MZ header)")
    pe_off = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_off : pe_off + 4] != b"PE\0\0":
        raise ValueError("not a PE file (missing PE signature)")
    coff = pe_off + 4
    num_sections = struct.unpack_from("<H", data, coff + 2)[0]
    opt_size = struct.unpack_from("<H", data, coff + 16)[0]
    opt_off = coff + 20
    magic = struct.unpack_from("<H", data, opt_off)[0]
    if magic == 0x10B:
        dir_off = opt_off + 96
    elif magic == 0x20B:
        dir_off = opt_off + 112
    else:
        raise ValueError(f"unknown optional-header magic 0x{magic:X}")
    res_rva, res_size = struct.unpack_from("<II", data, dir_off + 8 * IMAGE_DIRECTORY_ENTRY_RESOURCE)
    section_off = opt_off + opt_size
    sections = []
    for i in range(num_sections):
        base = section_off + i * 40
        vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, base + 8)
        sections.append((vaddr, max(vsize, rawsize), rawptr, rawsize))
    return res_rva, res_size, sections


def rva_to_offset(rva, sections):
    for vaddr, vsize, rawptr, rawsize in sections:
        if vaddr <= rva < vaddr + vsize:
            delta = rva - vaddr
            if delta < rawsize:
                return rawptr + delta
            return None
    return None


def resource_type_ids(data, res_rva, sections):
    """Integer IDs of the first resource level (the resource types)."""
    base = rva_to_offset(res_rva, sections)
    if base is None:
        return set()
    named, ids = struct.unpack_from("<HH", data, base + 12)
    types = set()
    for i in range(named + ids):
        entry = base + 16 + i * 8
        if entry + 8 > len(data):
            break
        name_field = struct.unpack_from("<I", data, entry)[0]
        if not (name_field & 0x80000000):
            types.add(name_field)
    return types


def type_entry_offset(data, res_rva, sections, type_id):
    """Offset of one resource-type directory, or None when the type is absent."""
    base = rva_to_offset(res_rva, sections)
    if base is None:
        return None
    named, ids = struct.unpack_from("<HH", data, base + 12)
    for i in range(named + ids):
        entry = base + 16 + i * 8
        if entry + 8 > len(data):
            break
        name_field, off_field = struct.unpack_from("<II", data, entry)
        if not (name_field & 0x80000000) and name_field == type_id:
            return base + (off_field & 0x7FFFFFFF)
    return None


def name_ids_at(data, directory):
    """Integer name/ID entries at the first level of a resource directory."""
    if directory is None or directory + 16 > len(data):
        return set()
    named, ids = struct.unpack_from("<HH", data, directory + 12)
    out = set()
    for i in range(named + ids):
        entry = directory + 16 + i * 8
        if entry + 8 > len(data):
            break
        name_field = struct.unpack_from("<I", data, entry)[0]
        if not (name_field & 0x80000000):
            out.add(name_field)
    return out


def group_icon_ids(data, res_rva, sections):
    """Resource IDs of the RT_GROUP_ICON entries (the icon group IDs)."""
    return name_ids_at(data, type_entry_offset(data, res_rva, sections, RT_GROUP_ICON))


def main():
    if len(sys.argv) != 2:
        print("usage: python scripts/check_windows_icon.py <piwidget.exe>", file=sys.stderr)
        return 2
    path = sys.argv[1]
    try:
        with open(path, "rb") as handle:
            data = handle.read()
    except OSError as err:
        print(f"[FAIL] cannot read {path}: {err}", file=sys.stderr)
        return 2

    try:
        res_rva, _res_size, sections = read_pe(data)
    except ValueError as err:
        print(f"[FAIL] {path}: {err}", file=sys.stderr)
        return 1

    if res_rva == 0:
        print(f"[FAIL] {path}: no .rsrc section - the exe has no icon resources", file=sys.stderr)
        return 1

    types = resource_type_ids(data, res_rva, sections)
    groups = group_icon_ids(data, res_rva, sections)
    icon_ids = name_ids_at(data, type_entry_offset(data, res_rva, sections, RT_ICON))

    ok = True
    if RT_ICON not in types:
        print(f"[FAIL] {path}: missing RT_ICON (type 3) resources", file=sys.stderr)
        ok = False
    if RT_GROUP_ICON not in types:
        print(f"[FAIL] {path}: missing RT_GROUP_ICON (type 14) resources", file=sys.stderr)
        ok = False
    # GPUI loads the icon by ID 1 only, so any other ID means no icon.
    if RT_GROUP_ICON in types and 1 not in groups:
        print(
            f"[FAIL] {path}: icon group resource ID is {sorted(groups)}, "
            f"but GPUI only loads ID 1",
            file=sys.stderr,
        )
        ok = False

    # A single-size icon blurs when scaled; Windows expects multiple sizes.
    if RT_ICON in types and len(icon_ids) < 4:
        print(
            f"[FAIL] {path}: only {len(icon_ids)} icon size(s), need at least 4 "
            f"(ids={sorted(icon_ids)})",
            file=sys.stderr,
        )
        ok = False

    if not ok:
        return 1

    print(
        f"[OK] {path}: icon resources present "
        f"(RT_ICON x{len(icon_ids)} + RT_GROUP_ICON id=1, {len(data)} bytes)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
