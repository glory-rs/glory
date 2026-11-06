"""Fresh engine checkouts must support locked tests without committed locks."""
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("engine_checker", Path(__file__).with_name("check-engine-forks.py"))
checker = importlib.util.module_from_spec(spec)
spec.loader.exec_module(checker)


class EngineLockfileTests(unittest.TestCase):
    def test_fresh_engine_gets_a_lock_and_existing_lock_is_preserved(self):
        with tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR")) as directory:
            root = Path(directory).resolve()
            target = root / "artifacts"
            (root / "src").mkdir()
            (root / "src/lib.rs").write_text("pub fn ready() -> bool { true }\n", encoding="utf-8")
            (root / "Cargo.toml").write_text(
                '[workspace]\n[package]\nname="engine-lock-fixture"\nversion="0.0.0"\n'
                'edition="2024"\npublish=false\n', encoding="utf-8")
            origin = "https://example.invalid/engine-lock-fixture"
            subprocess.run(["git", "init", "--quiet", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "remote", "add", "origin", origin], check=True)
            fork = {"path": ".", "git": origin, "tests": ["--lib"]}
            lock = root / "Cargo.lock"
            self.assertFalse(lock.exists())
            with patch.dict(os.environ, {"CARGO_TARGET_DIR": str(target), "CARGO_NET_OFFLINE": "true"}):
                checker.test_fork(fork, root, target, False)
                saved = lock.read_bytes()
                checker.test_fork(fork, root, target, False)
            self.assertEqual(lock.read_bytes(), saved)

    def test_engine_uses_its_own_configuration_when_called_from_another_workspace(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            (root / "src").mkdir()
            (root / "src/lib.rs").write_text("#[test] fn ready() {}\n", encoding="utf-8")
            (root / "Cargo.toml").write_text(
                '[workspace]\n[package]\nname="engine-own-target"\nversion="0.0.0"\n'
                'edition="2024"\npublish=false\n', encoding="utf-8")
            (root / ".cargo").mkdir()
            (root / ".cargo/config.toml").write_text('[build]\ntarget-dir="own-target"\n', encoding="utf-8")
            origin = "https://example.invalid/engine-own-target"
            subprocess.run(["git", "init", "--quiet", str(root)], check=True)
            subprocess.run(["git", "-C", str(root), "remote", "add", "origin", origin], check=True)
            with patch.dict(os.environ):
                os.environ.pop("CARGO_TARGET_DIR", None)
                checker.test_fork({"path": ".", "git": origin, "tests": ["--lib"]},
                                  root, root / "caller-target", False)
            self.assertTrue((root / "own-target/debug/deps").is_dir())
            self.assertFalse((root / "caller-target").exists())


if __name__ == "__main__":
    unittest.main()
