#!/usr/bin/env python3
"""Package a native macOS grok-pi binary and relocate its non-system dylibs.

Usage: python3 scripts/package-macos.py /path/to/grok-pi dist/asset.tar.gz
"""

import shutil
import subprocess
import sys
import tarfile
import tempfile
from pathlib import Path


def run(*args):
    return subprocess.check_output(args, text=True).strip()


def dependencies(path):
    return [line.strip().split(" (", 1)[0] for line in run("otool", "-L", str(path)).splitlines()[1:]]


def system(path):
    return path.startswith(("/usr/lib/", "/System/Library/"))


def package(binary, archive):
    with tempfile.TemporaryDirectory(prefix="grok-pi-macos-") as tmp:
        stage = Path(tmp)
        target = stage / "grok-pi"
        shutil.copy2(binary, target)
        libdir = stage / "lib/grok-pi"
        # Copy the dependency closure before changing any install names.
        pending = [binary.resolve()]
        libraries = {}
        while pending:
            original = pending.pop()
            for dep in dependencies(original):
                if system(dep) or Path(dep) == original or dep in libraries:
                    continue
                source = Path(dep)
                if not source.is_absolute() or not source.is_file():
                    raise ValueError(f"Unresolved dependency: {dep}")
                if any(p.name == source.name for p in libraries.values()):
                    raise ValueError(f"Duplicate dylib basename: {dep}")
                libdir.mkdir(parents=True, exist_ok=True)
                dest = libdir / source.name
                shutil.copy2(source, dest)
                libraries[dep] = dest
                pending.append(source)
        if libraries:
            notices = Path(__file__).resolve().parent.parent / "third_party/macos"
            shutil.copytree(notices, libdir / "licenses")
        for path in [*libraries.values(), target]:
            path.chmod(0o755)
            prefix = "@executable_path/lib/grok-pi/" if path == target else "@loader_path/"
            for dep in dependencies(path):
                if dep in libraries:
                    run("install_name_tool", "-change", dep, prefix + libraries[dep].name, str(path))
            if path != target:
                run("install_name_tool", "-id", "@loader_path/" + path.name, str(path))
            run("codesign", "--force", "--sign", "-", str(path))
            run("codesign", "--verify", "--strict", str(path))
            assert all(system(d) or d.startswith(("@executable_path/", "@loader_path/")) for d in dependencies(path))
        assert "grok-pi" in run(str(target), "--version")
        run(str(target), "--help")
        archive.parent.mkdir(parents=True, exist_ok=True)
        with tarfile.open(archive, "w:gz") as tar:
            tar.add(target, arcname="grok-pi")
            if libraries:
                tar.add(stage / "lib", arcname="lib")
        print(f"Packaged {archive}: {len(libraries)} relocated dylibs")


if __name__ == "__main__":
    package(Path(sys.argv[1]), Path(sys.argv[2]))
