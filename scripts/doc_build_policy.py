"""Validate current build instructions without rewriting historical evidence."""
import re


CURRENT_GUIDES = ("docs/release-readiness.md", "tools/cli/README.md")


def validate_build_guidance(text, source):
    portable = text.replace("\\", "/").lower()
    retired = ("d:/buildcache", "/mnt/d/buildcache", ".codex/build-targets")
    if any(path in portable for path in retired):
        raise ValueError(f"{source}: current instructions reference a retired build root")
    if re.search(r"set all cargo/cache/temp paths|isolates cargo/tool/app/temp|"
                 r"launchers set cargo caches|verified d: mount", portable):
        raise ValueError(f"{source}: current instructions redirect caller build paths")
    for required in ("inherit", "caller", "cargo metadata", "target_directory"):
        if required not in portable:
            raise ValueError(f"{source}: missing caller environment or metadata guidance ({required})")
