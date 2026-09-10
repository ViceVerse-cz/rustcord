# Debian / Ubuntu packages

On a Debian/Ubuntu Linux build host, `cargo xtask package` produces the text-only
`dist/serein_<version>-1_<architecture>.deb`; `cargo xtask package-voice` produces
`dist/voice/serein_<version>-1_<architecture>.deb`. Native `amd64` and `arm64`
ELF headers are accepted; a build does not prove desktop or audio support on that
architecture. These are unsigned host-distribution packages, not portable Linux
archives or a promise of compatibility with older distributions.

Install the native source-build dependencies in `docs/platform-support.md`, plus
`python3 dpkg-dev desktop-file-utils`. The packager uses Python's standard library,
`dpkg-shlibdeps`, `dpkg-deb`, `desktop-file-validate`, and `ldd`. It needs no root
privileges. It creates a fresh temporary staging tree on the native Linux
filesystem, normalizes file permissions and desktop-file line endings, and copies
only current documentation/license/source paths from the staged release output.
It excludes stale archives, nested voice outputs, logs, and stale license/source
files. Temporary files are removed when packaging finishes or raises an error.

The archive installs `/usr/bin/serein`, a launcher in
`/usr/share/applications/serein.desktop`, and documentation, notices, licenses and
applicable modified component source under `/usr/share/doc/serein`. No maintainer
scripts, background updater, automatic launch or user-profile writes are added.
Both variants have the **same package identity and version** and replace each other.
For a deliberate manual installation or variant switch, use the chosen local file:

```sh
sudo apt install --reinstall ./dist/serein_0.1.0-1_amd64.deb
# Or the optional voice build:
sudo apt install --reinstall ./dist/voice/serein_0.1.0-1_amd64.deb
```

These are user installation instructions; the build and smoke checks do not run
them. `--reinstall` ensures that switching variants at the same version is not
skipped. Remove the application with `sudo apt remove serein`; normal package
removal does not delete account data. Use in-app logout/cache controls as described
in the storage policy.

Dependencies are derived from the actual ELF using the host distribution's
installed shared-library symbols metadata. Missing libraries or dependency
metadata fail packaging. Explicit dependencies additionally cover dynamically
loaded Vulkan/EGL, X11/Wayland libraries and the D-Bus/desktop-portal services that
ELF inspection cannot discover. A working graphical session, graphics driver,
portal backend and unlocked Secret Service provider are still necessary for the
corresponding features. The package recommends a GTK or KDE portal backend and
GNOME Keyring; an existing compatible provider can be used instead. GTK/WebKit
and optional voice library requirements come from the built executable. The
resulting version constraints target the build distribution; inspect `Depends`
with `dpkg-deb --field <package.deb> Depends` before distributing elsewhere.

Every package is inspected before being copied to `dist`: metadata, allowed file
paths, root ownership, executable/data permissions, exact contents, absence of
maintainer scripts, valid desktop syntax and the host ELF library closure must
pass. This does not launch the application, install the package, access credentials,
initialize audio, or prove Wayland/X11, authentication, accessibility or live calls.
Run the small additional regression check without building the application:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 packaging/linux/test_package.py
```

It packages `/bin/true` with synthetic documentation and repository license files,
checks text/voice separation and rejection of stale nested payloads, then checks
mismatched payload and invalid ELF detection. The fixture is not a Serein build.

Tool contracts: [dpkg-shlibdeps](https://manpages.debian.org/trixie/dpkg-dev/dpkg-shlibdeps.1.en.html)
and [dpkg-deb](https://manpages.debian.org/trixie/dpkg/dpkg-deb.1.en.html).
