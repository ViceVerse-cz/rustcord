"""Synthetic Debian package regression check; never launches Serein or installs it."""

from pathlib import Path
import shutil
import tempfile
import unittest

import package as packaging


class DebianPackageTest(unittest.TestCase):
    def test_package_allowlist_and_corrupt_archive_detection(self):
        with tempfile.TemporaryDirectory(prefix="serein-debian-test-") as directory:
            root = Path(directory)
            staged = root / "staged"
            staged.mkdir()
            shutil.copyfile("/bin/true", staged / "serein")
            for name in ["README.md", "LICENSE-MIT", "LICENSE-APACHE", "THIRD_PARTY_NOTICES.md"]:
                (staged / name).write_text("synthetic package fixture\n")
            (staged / "docs").mkdir()
            for source in Path("docs").glob("*.md"):
                (staged / "docs" / source.name).write_text("synthetic documentation\n")
            (staged / "licenses").mkdir()
            for name in ["NotoSansCJK-LICENSE.txt", "NotoSansArabic-OFL.txt", "Inter-OFL.txt",
                         "Twemoji-CC-BY-4.0.txt", "Unicode-LICENSE.txt", "Phosphor-Icons-MIT.txt", "Simple-Icons-CC0.txt"]:
                (staged / "licenses" / name).write_text("synthetic license\n")
            for name in ["licenses/files", "licenses/notifications", "licenses/login",
                         "licenses/voice", "licenses/audio", "licenses/dependencies", "source/hpke-rs"]:
                source = Path("vendor/hpke-rs") if name.startswith("source/") else Path("assets") / name
                shutil.copytree(source, staged / name)
                (staged / name / "stale-nested.log").write_text("synthetic private marker\n")
            # Simulate a dirty dist directory: none of these belong to the archive.
            (staged / "voice").mkdir()
            (staged / "voice/stale.deb").write_text("old package")
            (staged / "debug.log").write_text("synthetic private marker")
            (staged / "docs/stale.log").write_text("synthetic private marker")
            (staged / "previous.deb").write_text("old package")
            packaging.package(staged, "0.1.0-test")
            artifact = next(staged.glob("serein_*.deb"))
            self.assertEqual(packaging.output("dpkg-deb", "--field", str(artifact), "Package"), "serein")
            listing = packaging.output("dpkg-deb", "--contents", str(artifact))
            for excluded in ["debug.log", "stale.log", "stale.deb", "previous.deb", "stale-nested.log"]:
                self.assertNotIn(excluded, listing)
            self.assertIn("licenses/dependencies/PROVENANCE.md", listing)
            self.assertIn("licenses/voice/", listing)
            self.assertIn("source/hpke-rs/Cargo.toml", listing)
            # Preserve valid metadata while making the expected payload disagree.
            wrong_stage = root / "wrong-stage"
            wrong_stage.mkdir()
            check = root / "check"
            check.mkdir()
            with self.assertRaisesRegex(ValueError, "payload differs"):
                packaging.smoke(
                    artifact, wrong_stage, check, "0.1.0~test-1",
                    packaging.output("dpkg", "--print-architecture"),
                    packaging.output("dpkg-deb", "--field", str(artifact), "Depends"))
            (staged / "serein").write_bytes(b"MZ synthetic wrong architecture")
            with self.assertRaisesRegex(ValueError, "ELF executable"):
                packaging.package(staged, "0.1.0")


if __name__ == "__main__":
    unittest.main()
