"""Regression tests for the labeled-image fixture generator.

Run: python -m unittest discover -s experiments -p "test_*.py" -v
Requires Pillow. All generated files live in temporary directories.
"""
import csv
import hashlib
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

from PIL import Image

SCRIPT = Path(__file__).with_name("generar_pruebas_photovault.py")


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


class FixtureGeneratorTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.source = self.root / "originals"
        self.source.mkdir()
        for index in range(5):
            image = Image.new("RGB", (96, 80), (index * 35, 80, 140))
            image.save(self.source / f"foto_{index}.png")
        self.before = {p.name: sha256(p) for p in self.source.iterdir()}

    def run_generator(self):
        return subprocess.run(
            [sys.executable, str(SCRIPT), str(self.source)],
            capture_output=True, text=True, check=False,
        )

    def test_creates_65_images_and_manifest_without_modifying_sources(self):
        result = self.run_generator()
        self.assertEqual(result.returncode, 0, result.stderr)
        output = self.root / "originals_PhotoVault_Test"
        self.assertTrue(output.is_dir())
        photos = sorted(p for p in output.iterdir() if p.suffix.lower() in {".jpg", ".png"})
        self.assertEqual(len(photos), 65)
        with (output / "manifest.csv").open(encoding="utf-8-sig", newline="") as handle:
            rows = list(csv.DictReader(handle))
        self.assertEqual(len(rows), 65)
        self.assertEqual(len({r["grupo"] for r in rows}), 5)
        self.assertEqual({r["grupo"] for r in rows}, {"F01", "F02", "F03", "F04", "F05"})
        for row in rows:
            self.assertEqual(row["sha256"], sha256(output / row["archivo"]))
        for group in ("F01", "F02", "F03", "F04", "F05"):
            exact = [r for r in rows if r["grupo"] == group and r["transformacion"] in {"original", "COPIA_EXACTA", "RENOMBRADO_EXACTO"}]
            self.assertEqual(len(exact), 3)
            self.assertEqual(len({r["sha256"] for r in exact}), 1)
        self.assertEqual(self.before, {p.name: sha256(p) for p in self.source.iterdir()})

    def test_refuses_to_overwrite_existing_output(self):
        self.assertEqual(self.run_generator().returncode, 0)
        output = self.root / "originals_PhotoVault_Test"
        marker = output / "do_not_touch.txt"
        marker.write_text("preserve", encoding="utf-8")
        second = self.run_generator()
        self.assertNotEqual(second.returncode, 0)
        self.assertEqual(marker.read_text(encoding="utf-8"), "preserve")
        self.assertEqual(self.before, {p.name: sha256(p) for p in self.source.iterdir()})

    def test_requires_exactly_five_originals(self):
        (self.source / "foto_4.png").unlink()
        result = self.run_generator()
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse((self.root / "originals_PhotoVault_Test").exists())


if __name__ == "__main__":
    unittest.main()
