"""Keep release documentation linked to the active declarations, not old counts."""
import json
from pathlib import Path
import re
import tomllib

ROOT = Path(__file__).resolve().parents[1]
manifest = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
plan = json.loads((ROOT / "release.json").read_text(encoding="utf-8"))
readme = (ROOT / "README.md").read_text(encoding="utf-8")
readiness = (ROOT / "docs/release-readiness.md").read_text(encoding="utf-8")
guide = (ROOT / "docs/api-guide.md").read_text(encoding="utf-8")
assert "[`FORMAT_VERSION`](crates/html/src/lib.rs)" in readme
assert "[`STYLE_PROFILE`](crates/apace/src/document.rs)" in readme
assert "release.json" in readiness and "twelve-package" not in readiness
assert "separately reviewed current pipeline" not in readiness
assert "crates/blitz" not in guide and "link_with" in guide and "Selected<K>" in guide
assert (ROOT / ".github/workflows/release.yml").is_file()
names = []
for member in manifest["workspace"]["members"]:
    package = tomllib.loads((ROOT / member / "Cargo.toml").read_text(encoding="utf-8"))["package"]
    names.append(package["name"])
assert set(names).issubset(plan["packages"])
for relative in ("crates/html/src/lib.rs", "crates/apace/src/document.rs"):
    source = (ROOT / relative).read_text(encoding="utf-8")
    assert re.search(r"FORMAT_VERSION: u32 = \d+|pub const STYLE_PROFILE:", source), relative
print(f"Current documentation references verified for {len(names)} workspace packages")
