"""Build and inspect an unsigned Debian package from xtask's staged release files.

No installation or application launch. Uses only Python's standard library and
Debian packaging/desktop tools; run through cargo xtask package[-voice].
"""

import filecmp
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile


def output(*args, cwd=None):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def checked(*args, cwd=None):
    subprocess.run(args, cwd=cwd, check=True)


def copy(source, destination, manifest=None):
    manifest = source if manifest is None else manifest
    if source.is_symlink() or manifest.is_symlink():
        raise ValueError(f"Refusing symlink in package input: {source}")
    if source.is_dir():
        destination.mkdir(parents=True, exist_ok=True)
        for child in sorted(manifest.iterdir()):
            copy(source / child.name, destination / child.name, child)
    else:
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)


def payload_files(root):
    return sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file())


def smoke(package, stage, temporary, version, architecture, depends):
    fields = {
        "Package": "serein", "Version": version, "Architecture": architecture,
        "Depends": depends,
    }
    for field, expected in fields.items():
        actual = output("dpkg-deb", "--field", str(package), field)
        if actual != expected:
            raise ValueError(f"Package {field}: expected {expected!r}, got {actual!r}")
    # Inspect before extraction; never extract links, device nodes or traversal paths.
    archive = temporary / "payload.tar"
    with archive.open("wb") as stream:
        subprocess.run(["dpkg-deb", "--fsys-tarfile", str(package)], stdout=stream, check=True)
    with tarfile.open(archive) as contents:
        for member in contents:
            path = Path(member.name)
            expected_mode = 0o755 if member.isdir() or member.name == "./usr/bin/serein" else 0o644
            if (path.is_absolute() or ".." in path.parts
                    or not (member.isfile() or member.isdir())
                    or member.mode != expected_mode or member.uid or member.gid):
                raise ValueError(f"Unsafe package entry or incorrect mode/owner: {member.name}")
    extracted = temporary / "extracted"
    checked("dpkg-deb", "--extract", str(package), str(extracted))
    expected = [name for name in payload_files(stage) if not name.startswith("DEBIAN/")]
    if payload_files(extracted) != expected:
        raise ValueError("Package payload differs from staged allowlist")
    for name in expected:
        if not filecmp.cmp(stage / name, extracted / name, shallow=False):
            raise ValueError(f"Package changed file contents: {name}")
    control = temporary / "extracted-control"
    checked("dpkg-deb", "--control", str(package), str(control))
    if payload_files(control) != ["control"]:
        raise ValueError("Unexpected control files or maintainer scripts")
    checked("desktop-file-validate", str(extracted / "usr/share/applications/serein.desktop"))
    libraries = output("ldd", str(extracted / "usr/bin/serein"))
    if "not found" in libraries:
        raise ValueError(f"Unresolved packaged executable dependencies:\n{libraries}")
    print(f"Debian package smoke passed: {len(expected)} files; executable, desktop entry, "
          "metadata, ownership, content and host shared-library closure verified.")


def package(root, application_version, variant):
    if sys.platform != "linux":
        raise ValueError("Debian packaging must run natively on Debian/Ubuntu Linux")
    architecture = output("dpkg", "--print-architecture")
    machine = {"amd64": 62, "arm64": 183}.get(architecture)
    if machine is None:
        raise ValueError(f"Unverified Debian package architecture: {architecture}")
    # Rust and dpkg must agree: do not label a cross-built binary as the host arch.
    with (root / "serein").open("rb") as executable:
        header = executable.read(20)
    if (len(header) != 20 or header[:6] != b"\x7fELF\x02\x01"
            or struct.unpack("<H", header[18:20])[0] != machine):
        raise ValueError("Expected a native little-endian 64-bit ELF executable")
    version = application_version.replace("-", "~", 1) + "-1"
    checked("dpkg", "--validate-version", version)
    # A fresh owned directory prevents stale voice files, logs or credentials in dist
    # from entering a text package. TemporaryDirectory removes only this invocation.
    with tempfile.TemporaryDirectory(prefix="serein-debian-package-") as directory:
        temporary = Path(directory).resolve()
        stage = temporary / "debian/serein"
        doc = stage / "usr/share/doc/serein"
        copy(root / "serein", stage / "usr/bin/serein")
        desktop = stage / "usr/share/applications/serein.desktop"
        desktop.parent.mkdir(parents=True)
        desktop.write_text(Path("packaging/linux/serein.desktop").read_text(), encoding="utf-8")
        for name in ["README.md", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md"]:
            copy(root / name, doc / name)
        for source in sorted(Path("docs").glob("*.md")):
            copy(root / "docs" / source.name, doc / "docs" / source.name)
        for name in ["NotoSansCJK-LICENSE.txt", "NotoSansArabic-OFL.txt", "Inter-OFL.txt",
                     "Twemoji-CC-BY-4.0.txt", "Unicode-LICENSE.txt", "Phosphor-Icons-MIT.txt", "Simple-Icons-CC0.txt",
                     ]:
            copy(root / "licenses" / name, doc / "licenses" / name)
        for name in ["files", "notifications", "login", "audio", *(["voice"] if variant == "voice" else [])]:
            copy(root / "licenses" / name, doc / "licenses" / name, Path("assets/licenses") / name)
        if variant == "voice":
            copy(root / "source/hpke-rs", doc / "source/hpke-rs", Path("vendor/hpke-rs"))
        debian = temporary / "debian"
        debian.mkdir(exist_ok=True)
        (debian / "control").write_text(
            "Source: serein\nSection: net\nPriority: optional\n"
            "Maintainer: Serein contributors <noreply@github.com>\n\n"
            "Package: serein\nArchitecture: any\nDescription: Unofficial native Discord client\n",
            encoding="utf-8")
        (stage / "DEBIAN").mkdir()
        dependencies = output("dpkg-shlibdeps", "-O", "debian/serein/usr/bin/serein", cwd=temporary)
        depends = next(line.removeprefix("shlibs:Depends=") for line in dependencies.splitlines()
                       if line.startswith("shlibs:Depends="))
        # dlopen libraries and desktop services are invisible to ELF DT_NEEDED.
        depends += (", libvulkan1, libegl1, libxkbcommon0, libxkbcommon-x11-0, "
                    "libwayland-client0, libx11-6, libx11-xcb1, libxcursor1, libxi6, libxrandr2, "
                    "dbus-user-session | dbus-x11, xdg-desktop-portal")
        installed_kib = sum(1 if p.is_dir() else max(1, (p.stat().st_size + 1023) // 1024)
                            for p in stage.rglob("*"))
        control = stage / "DEBIAN/control"
        control.write_text(
            f"Package: serein\nVersion: {version}\nArchitecture: {architecture}\n"
            "Section: net\nPriority: optional\n"
            "Maintainer: Serein contributors <noreply@github.com>\n"
            "Homepage: https://github.com/ViceVerse-cz/rustcord\n"
            f"Installed-Size: {installed_kib}\nDepends: {depends}\n"
            "Recommends: gnome-keyring, xdg-desktop-portal-gtk | xdg-desktop-portal-kde\n"
            f"Description: Unofficial native Discord client ({variant} build)\n"
            " Native Rust desktop client for existing Discord accounts.\n"
            " Unofficial, experimental, and not endorsed by Discord.\n",
            encoding="utf-8")
        for path in [stage, *stage.rglob("*")]:
            path.chmod(0o755 if path.is_dir() or path == stage / "usr/bin/serein" else 0o644)
        checked("desktop-file-validate", str(stage / "usr/share/applications/serein.desktop"))
        artifact = root / f"serein_{version}_{architecture}.deb"
        candidate = temporary / artifact.name
        checked("dpkg-deb", "--root-owner-group", "-Zxz", "--build", str(stage), str(candidate))
        smoke(candidate, stage, temporary, version, architecture, depends)
        shutil.copyfile(candidate, artifact)
        print(f"Unsigned {variant} Debian package: {artifact} ({artifact.stat().st_size} bytes; "
              f"Installed-Size {installed_kib} KiB). Same package identity replaces the other variant.")
        print(f"Runtime dependencies: {depends}; recommends a Secret Service provider (gnome-keyring).")


if __name__ == "__main__":
    if len(sys.argv) != 4 or sys.argv[3] not in ("text", "voice"):
        sys.exit("Usage: package.py STAGED_DIRECTORY APPLICATION_VERSION text|voice")
    package(Path(sys.argv[1]).resolve(), sys.argv[2], sys.argv[3])
