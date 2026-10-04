import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("generator", Path(__file__).with_name("generate-homebrew.py"))
generator = importlib.util.module_from_spec(spec)
spec.loader.exec_module(generator)


class PackagingTests(unittest.TestCase):
    def test_both_architectures_use_their_own_checksums(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = "\n".join(f"{str(n) * 64}  harnesscope-v1.2.3-{target}.tar.gz" for n, target in enumerate(
                ["aarch64-apple-darwin", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu"], 1))
            generator.generate("v1.2.3", manifest, root)
            cask = (root / "Casks/harnesscope.rb").read_text()
            self.assertIn('version "1.2.3"', cask)
            self.assertIn(f'arm:   "{"1" * 64}"', cask)
            self.assertIn(f'intel: "{"2" * 64}"', cask)
            self.assertIn('binary "harnesscope"', cask)
            self.assertNotIn("no_quarantine", cask)
            self.assertIn("#{arch}", cask)
            self.assertIn("3" * 64, (root / "Formula/harnesscope.rb").read_text())

    def test_invalid_inputs_leave_existing_definitions_unchanged(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "Formula").mkdir()
            formula = root / "Formula/harnesscope.rb"
            formula.write_text("preserve")
            for version, manifest in [("v1.2.3", ""), ('v1.2.3";command', ""), ("v1.2.3", "bad manifest")]:
                with self.assertRaises(ValueError):
                    generator.generate(version, manifest, root)
                self.assertEqual(formula.read_text(), "preserve")


if __name__ == "__main__":
    unittest.main()
