"""Checks a Windows release of Worldline needs nothing but Windows (step 8.8a).

    python tools/check_exe.py target/release/worldline.exe

Reads the program's own header (the PE format Windows loads it by) and checks:
- it's a 64-bit x86 program;
- it's a windowed program, so no console window opens behind it;
- every DLL it loads, as it starts or later (delay-loaded), is part of
  Windows: none from the Visual C++ runtime (vcruntime140.dll, msvcp140.dll
  and the like), which a fresh computer doesn't have unless something
  installed it. The release build links that runtime into the program.

On Windows it also checks each of those DLLs is in Windows' own System32
folder, or is one of the API sets Windows resolves itself (api-ms-win-*).

Python's standard library only. See docs/release.md.
"""

import os
import struct
import sys
from pathlib import Path

# The Visual C++ runtime: not part of Windows, so a fresh computer may not
# have it.
VISUAL_CPP = ("vcruntime", "msvcp", "concrt", "vcomp", "vccorlib")
# The Universal C runtime: part of Windows 10 and 11, but the release build
# carries it inside, so loading it means the build wasn't static.
UNIVERSAL_C = ("ucrtbase", "api-ms-win-crt-")

MACHINES = {0x8664: "x86-64", 0x14C: "x86", 0xAA64: "ARM64"}
SUBSYSTEMS = {2: "windowed (no console)", 3: "console"}


class Pe:
    """The parts of a PE file's header this check reads."""

    def __init__(self, data):
        self.data = data
        if data[:2] != b"MZ":
            raise ValueError("not a Windows program (no MZ header)")
        header = struct.unpack_from("<I", data, 0x3C)[0]
        if data[header : header + 4] != b"PE\0\0":
            raise ValueError("not a Windows program (no PE signature)")
        coff = header + 4
        self.machine, sections, _, _, _, optional_size, _ = struct.unpack_from("<HHIIIHH", data, coff)
        optional = coff + 20
        magic = struct.unpack_from("<H", data, optional)[0]
        if magic != 0x20B:
            raise ValueError(f"not a 64-bit program (optional header magic {magic:#x})")
        self.subsystem = struct.unpack_from("<H", data, optional + 68)[0]
        count = struct.unpack_from("<I", data, optional + 108)[0]
        self.directories = [struct.unpack_from("<II", data, optional + 112 + 8 * i) for i in range(count)]
        table = optional + optional_size
        self.sections = []
        for i in range(sections):
            _, virtual_size, address, raw_size, raw_at = struct.unpack_from("<8sIIII", data, table + 40 * i)
            self.sections.append((address, max(virtual_size, raw_size), raw_at))

    def offset(self, rva):
        for address, size, raw_at in self.sections:
            if address <= rva < address + size:
                return rva - address + raw_at
        raise ValueError(f"address {rva:#x} is in no section")

    def name(self, rva):
        start = self.offset(rva)
        return self.data[start : self.data.index(b"\0", start)].decode("ascii")

    def imports(self):
        """The DLLs loaded as it starts."""
        if len(self.directories) < 2 or not self.directories[1][0]:
            return []
        at, names = self.offset(self.directories[1][0]), []
        while True:
            entry = struct.unpack_from("<IIIII", self.data, at)
            if not any(entry):
                return names
            names.append(self.name(entry[3]))
            at += 20

    def delay_imports(self):
        """The DLLs loaded the first time they're needed."""
        if len(self.directories) < 14 or not self.directories[13][0]:
            return []
        at, names = self.offset(self.directories[13][0]), []
        while True:
            entry = struct.unpack_from("<8I", self.data, at)
            if not any(entry):
                return names
            names.append(self.name(entry[1]))
            at += 32


def where(dll):
    """Where a DLL comes from, and whether that's a problem."""
    lower = dll.lower()
    if lower.startswith(VISUAL_CPP):
        return "the Visual C++ runtime: NOT PART OF WINDOWS", False
    if lower.startswith(UNIVERSAL_C):
        return "the C runtime, which the release build should carry inside: NOT STATIC", False
    source = system_dll(dll)
    return (source, True) if source else ("NOT PART OF WINDOWS", False)


def system_dll(dll):
    """Where in Windows a DLL comes from, or None."""
    lower = dll.lower()
    if lower.startswith(("api-ms-win-", "ext-ms-")):
        return "an API set Windows resolves"
    if os.name != "nt":
        return "not checked (not on Windows)"
    system = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
    return "Windows' System32" if (system / dll).is_file() else None


def main():
    for stream in (sys.stdout, sys.stderr):
        stream.reconfigure(errors="backslashreplace")
    if len(sys.argv) != 2:
        sys.exit(__doc__.split("\n\n")[1])
    path = Path(sys.argv[1])
    try:
        pe = Pe(path.read_bytes())
        loaded = [(dll, "as it starts") for dll in pe.imports()]
        loaded += [(dll, "when first needed") for dll in pe.delay_imports()]
    except (OSError, ValueError, struct.error) as e:
        sys.exit(f"check_exe: {path}: {e}")

    problems = []
    machine = MACHINES.get(pe.machine, f"machine {pe.machine:#x}")
    subsystem = SUBSYSTEMS.get(pe.subsystem, f"subsystem {pe.subsystem}")
    print(f"{path.name}: {path.stat().st_size / 1e6:.1f} MB, {machine}, {subsystem}")
    if pe.machine != 0x8664:
        problems.append(f"it's for {machine}, not x86-64")
    if pe.subsystem != 2:
        problems.append(f"it's a {subsystem} program, so a console window would open behind it")
    print(f"it loads {len(loaded)} DLLs:")
    for dll, when in sorted(loaded, key=lambda d: d[0].lower()):
        source, fine = where(dll)
        print(f"  {dll} ({when}): {source}")
        if not fine:
            problems.append(f"it loads {dll}")
    if problems:
        sys.exit("check_exe: FAILED: " + "; ".join(problems))
    print("passed: it needs nothing but Windows")


if __name__ == "__main__":
    main()
