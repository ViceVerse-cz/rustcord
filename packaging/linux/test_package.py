"""Synthetic Debian package regression check; never launches Serein or installs it."""

import json
from pathlib import Path
import shutil
import tempfile
import unittest

import package as packaging


class DebianPackageTest(unittest.TestCase):
    def test_dependency_inventory_rejects_invalid_files_and_paths(self):
        with tempfile.TemporaryDirectory(prefix="serein-inventory-test-") as directory:
            root = Path(directory)
            source = root / "input"
            source.mkdir()
            (source / "LICENSE").write_text("synthetic license\n")
            destination = root / "output"
            inventory = source / "inventory.json"
            for name in ["../outside", "/absolute", "sub/../../outside", "sub//LICENSE",
                         "./LICENSE", "C:/LICENSE", "sub\\LICENSE", "inventory.json", "missing"]:
                with self.subTest(name=name):
                    inventory.write_text(json.dumps({"schema_version": 1, "files": [name]}))
                    with self.assertRaises((ValueError, FileNotFoundError)):
                        packaging.copy_dependency_notices(source, destination)
            inventory.write_text(json.dumps({"schema_version": 1, "files": ["LICENSE", "LICENSE"]}))
            with self.assertRaisesRegex(ValueError, "inventory path"):
                packaging.copy_dependency_notices(source, destination)
            for invalid in [{}, [], {"schema_version": True, "files": ["LICENSE"]},
                            {"schema_version": 1, "files": ["LICENSE"] * 8193}]:
                inventory.write_text(json.dumps(invalid))
                with self.assertRaisesRegex(ValueError, "schema or file count"):
                    packaging.copy_dependency_notices(source, destination)
            with (source / "large").open("wb") as stream:
                stream.truncate(8 * 1024 * 1024 + 1)
            inventory.write_text(json.dumps({"schema_version": 1, "files": ["large"]}))
            with self.assertRaisesRegex(ValueError, "oversized"):
                packaging.copy_dependency_notices(source, destination)
            inventory.write_text(" " * (1024 * 1024 + 1))
            with self.assertRaisesRegex(ValueError, "oversized"):
                packaging.copy_dependency_notices(source, destination)
            inventory.write_text(json.dumps({"schema_version": 1, "files": ["link/LICENSE"]}))
            (source / "link").symlink_to(source, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "Symlink"):
                packaging.copy_dependency_notices(source, destination)
            (source / "link").unlink()
            inventory.write_text(json.dumps({"schema_version": 1, "files": ["LICENSE"]}))
            (source / "LICENSE").unlink()
            (source / "LICENSE").symlink_to(inventory)
            with self.assertRaisesRegex(ValueError, "Symlink"):
                packaging.copy_dependency_notices(source, destination)

    def test_variant_allowlist_and_corrupt_archive_detection(self):
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
                         "licenses/voice", "licenses/audio", "source/hpke-rs"]:
                source = Path("vendor/hpke-rs") if name.startswith("source/") else Path("assets") / name
                shutil.copytree(source, staged / name)
                (staged / name / "stale-nested.log").write_text("synthetic private marker\n")
            notices = staged / "licenses/dependencies"
            (notices / "synthetic-1.0.0").mkdir(parents=True)
            (notices / "synthetic-1.0.0/LICENSE").write_text("synthetic dependency license\n")
            (notices / "unlisted-private.log").write_text("synthetic private marker\n")
            (notices / "inventory.json").write_text(json.dumps({
                "schema_version": 1, "files": ["synthetic-1.0.0/LICENSE"],
            }))
            # Simulate a dirty dist directory: none of these belong to either archive.
            (staged / "voice").mkdir()
            (staged / "voice/stale.deb").write_text("old package")
            (staged / "debug.log").write_text("synthetic private marker")
            (staged / "docs/stale.log").write_text("synthetic private marker")
            (staged / "previous.deb").write_text("old package")
            for variant in ["text", "voice"]:
                packaging.package(staged, "0.1.0-test", variant)
                artifact = next(staged.glob("serein_*.deb"))
                self.assertEqual(packaging.output("dpkg-deb", "--field", str(artifact), "Package"), "serein")
                listing = packaging.output("dpkg-deb", "--contents", str(artifact))
                for excluded in ["debug.log", "stale.log", "stale.deb", "previous.deb", "stale-nested.log", "unlisted-private.log"]:
                    self.assertNotIn(excluded, listing)
                self.assertIn("licenses/dependencies/inventory.json", listing)
                self.assertIn("licenses/dependencies/synthetic-1.0.0/LICENSE", listing)
                self.assertEqual("licenses/voice/" in listing, variant == "voice")
                self.assertEqual("source/hpke-rs/Cargo.toml" in listing, variant == "voice")
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
                packaging.package(staged, "0.1.0", "text")


if __name__ == "__main__":
    unittest.main()
