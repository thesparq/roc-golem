#!/usr/bin/env python3
"""Build Rust host, rebuild demo, and produce roc-golem platform bundle."""

import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PLATFORM_DIR = ROOT / "platform"
TARGETS_DIR = PLATFORM_DIR / "targets" / "wasm32"
HOST_DIR = ROOT / "host"
OUTPUT_DIR = ROOT / "dist"


def build_rust_host() -> None:
    print("==> Building Rust host (wasm32-unknown-unknown)")
    subprocess.run(
        [
            "cargo", "rustc",
            "--target=wasm32-unknown-unknown",
            "--release",
            "--",
            "--emit=obj",
            "-C", "relocation-model=pic",
        ],
        cwd=str(HOST_DIR),
        check=False,
    )
    objs = sorted(Path(HOST_DIR / "target" / "wasm32-unknown-unknown" / "release" / "deps").glob("*golem_host*.o"))
    if not objs:
        raise SystemExit("No host object file found")
    obj = objs[-1]
    dest = TARGETS_DIR / "host.wasm"
    print(f"  -> {dest}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    obj.rename(dest)


def bundle_platform() -> str:
    print("==> Bundling platform")
    roc_files = sorted(PLATFORM_DIR.glob("*.roc"))
    lib_files = sorted(TARGETS_DIR.rglob("*"))
    bundle_files = [
        *[f.relative_to(PLATFORM_DIR).as_posix() for f in roc_files],
        *[f.relative_to(PLATFORM_DIR).as_posix() for f in lib_files if f.is_file()],
    ]
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    result = subprocess.run(
        ["roc", "bundle", *bundle_files, "--output-dir", "../dist"],
        cwd=str(PLATFORM_DIR),
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        print(result.stderr, file=sys.stderr)
        raise SystemExit("roc bundle failed")
    output_line = [l for l in result.stdout.splitlines() if l.startswith("Created:")][0]
    bundle_path = output_line.removeprefix("Created: ").strip()
    bundle_name = Path(bundle_path).name
    print(f"  -> dist/{bundle_name}")
    print(result.stdout.strip())
    return bundle_name


def main() -> None:
    build_rust_host()
    bundle_platform()


if __name__ == "__main__":
    main()
