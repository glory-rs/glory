"""Build isolation: consumer workspaces must never enter upstream defaults."""
from pathlib import Path as _BuildPath
import sys as _build_sys
_build_sys.path.insert(0, str(_BuildPath(__file__).resolve().parents[1] / "scripts"))
from local_build_paths import local_build_path
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import unittest
from unittest.mock import patch

from library_lockfiles import LIBRARY_MANIFESTS, ensure_library_lockfiles


class LockfileScopeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(dir=os.environ.get("TMPDIR"))
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name).resolve()
        self.target = self.root / "artifacts"
        self.environment = patch.dict(os.environ, {"CARGO_TARGET_DIR": str(self.target)})
        self.environment.start()
        self.addCleanup(self.environment.stop)
        self.assertTrue(self.target.is_relative_to(self.root))
        for relative in LIBRARY_MANIFESTS:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("[workspace]\n", encoding="utf-8")

    def metadata(self, arguments, **_):
        manifest = Path(arguments[arguments.index("--manifest-path") + 1])
        return json.dumps({"target_directory": str(self.target), "workspace_root": str(manifest.parent)})

    def test_unlisted_consumer_and_peer_workspaces_are_never_discovered(self):
        extra = self.root / "examples/consumer/Cargo.toml"
        extra.parent.mkdir(parents=True)
        extra.write_text('[workspace]\n[dependencies]\nconsumer = {path="/missing/downstream"}\n')
        with patch("library_lockfiles.subprocess.check_output", side_effect=self.metadata) as metadata, \
                patch("library_lockfiles.subprocess.run") as generate:
            ensure_library_lockfiles(self.root)
        observed = {Path(call.args[0][-1]).relative_to(self.root).as_posix() for call in metadata.call_args_list}
        self.assertEqual(observed, set(LIBRARY_MANIFESTS))
        self.assertEqual(generate.call_count, len(LIBRARY_MANIFESTS))

    def test_explicit_scope_and_existing_locks_do_not_touch_other_workspaces(self):
        (self.root / "Cargo.lock").write_text("version = 4\n")
        with patch("library_lockfiles.subprocess.check_output", side_effect=self.metadata) as metadata, \
                patch("library_lockfiles.subprocess.run") as generate:
            ensure_library_lockfiles(self.root, ["Cargo.toml"])
        self.assertEqual(metadata.call_count, 1)
        generate.assert_not_called()

    def test_invalid_scope_and_wrong_target_fail_before_resolution(self):
        with self.assertRaises(SystemExit):
            ensure_library_lockfiles(self.root, ["../Cargo.toml"])
        with patch("library_lockfiles.subprocess.check_output", return_value='{"target_directory":"/wrong","workspace_root":"/wrong"}'), \
                patch("library_lockfiles.subprocess.run") as generate:
            with self.assertRaises(SystemExit):
                ensure_library_lockfiles(self.root, ["Cargo.toml"])
        generate.assert_not_called()


class RepositoryBoundaryTests(unittest.TestCase):
    def test_every_declared_path_dependency_is_owned_or_a_declared_engine(self):
        root = Path(__file__).resolve().parents[1]
        engines = json.loads((root / "docs/engine-forks.json").read_text())["forks"]
        allowed = [root, *((root / engine["path"]).resolve() for engine in engines if engine.get("path"))]
        manifests = subprocess.check_output(["git", "ls-files", "*Cargo.toml"], cwd=root, text=True).splitlines()

        def dependencies(table):
            for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
                yield from table.get(kind, {}).values()

        for relative in manifests:
            manifest = root / relative
            data = tomllib.loads(manifest.read_text(encoding="utf-8"))
            declared = list(dependencies(data)) + list(dependencies(data.get("workspace", {})))
            for target in data.get("target", {}).values():
                declared.extend(dependencies(target))
            for source in data.get("patch", {}).values():
                declared.extend(source.values())
            for dependency in declared:
                if isinstance(dependency, dict) and "path" in dependency:
                    path = (manifest.parent / dependency["path"]).resolve()
                    self.assertTrue(any(path.is_relative_to(base) for base in allowed), f"Consumer dependency in {relative}: {path}")


if __name__ == "__main__":
    unittest.main()
