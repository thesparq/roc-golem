# Specification: Unit 15 — Release Polish, Packaging Integrity and Production Hardening

- **Unit:** 15
- **File:** `docs/specs/15-release-polish-and-hardening.md`
- **Status:** Complete
- **Phase:** Phase 3 — Release
- **Fixes:** `KI-20`, `KI-21`, `KI-22`, `KI-23`, `KI-24`, `KI-25`

---

## 1. Goal

Address all findings from the post-GA completeness review to ensure the Roc Golem Platform is 100% functional, downloadable, reproducible, and verifiable for any external developer:
1. **Packaging Integrity (`KI-20`)**: Fix `tooling/pack_platform/src/main.rs` to recursively include `platform/targets/wasm32/libroc_golem_host.a` in the `.tar.zst`, `.tar.br`, and `.tar.gz` distribution packages. Without this file at `targets/wasm32/`, the modern Roc platform pipeline fails with `missing target file`.
2. **Automated Consumer Verification (`KI-20`)**: Author `tooling/test_consumer.sh` (and integrate into CI) which extracts the packaged `.tar.zst` archive into an isolated temporary directory, compiles a real Roc agent against it, and validates component model compliance.
3. **Release Asset & Checksum Synchronization (`KI-21`)**: Re-generate the release archives, update GitHub release `v1.0.0` assets via `gh release upload --clobber`, and update `README.md` with the verified BLAKE3 base64url checksum so that `curl -fsSL <url>` returns HTTP 200 with matching integrity.
4. **Clean Roc Compiler Scope (`KI-22`)**: Remove redundant `import Json` in `platform/main.roc` to eliminate the `duplicate definition` compiler warning, achieving 0 errors and 0 warnings under `roc check`.
5. **Path, Name & Manifest Consistency (`KI-23`)**: Standardize component emission in `tooling/build.sh` so both `build/${app}.wasm` and `build/${app}_agent.wasm` exist; update `golem.yaml` to point to `build/streaming_agent_agent.wasm`; remove mentions of nonexistent `host.wasm` from `README.md` and packager; align README deploy commands with Golem CLI 1.2.5 syntax.
6. **Runtime Test Harness Resiliency (`KI-24`)**: Make `tooling/test_runtime.sh` robust when a local Golem server is already active on port 9881 by checking health, reusing or updating existing components, and using unique worker IDs to prevent collision errors.
7. **Checklist & Tracker Alignment (`KI-25`)**: Update `docs/feature-checklist.md` rows 15 and 16 to reflect verified status, and add explicit verification source tags (`[Live Golem Server (1.2.5)]` vs `[Host Unit Test (Mock)]`) across all checklist items for complete transparency.

---

## 2. Design & Architecture

### 2.1 The Packaging Problem (`KI-20`)
In `platform/main.roc`, the platform declares:
```roc
targets: {
    inputs_dir: "targets/",
    wasm32: {
        inputs: ["libroc_golem_host.a", app],
        output: Shared,
        exports: [...],
    },
}
```
When an external developer specifies:
```roc
app [agent] {
    pf: platform "https://github.com/thesparq/roc-golem/releases/download/v1.0.0/<hash>.tar.zst",
}
```
Roc downloads and unpacks the archive into `~/.cache/roc/`. When building for `wasm32`, Roc looks for the host library at `<unpacked_dir>/targets/wasm32/libroc_golem_host.a`.

Prior to this unit, `tooling/pack_platform/src/main.rs` only scanned top-level files with `if path.is_file()`, skipping subdirectories. Consequently, the release archive contained:
- `main.roc`, `Golem.roc`, `Types.roc`, `Effect.roc`, `Json.roc`, `LICENSE`, `README.md`, `libhost.a`
and completely omitted `targets/wasm32/libroc_golem_host.a`. External consumers attempting to build encountered `missing target file`.

### 2.2 Fix for Packager
In `tooling/pack_platform/src/main.rs`:
- Walk `platform_dir` recursively.
- Add all files preserving their relative paths inside the platform directory (e.g., `targets/wasm32/libroc_golem_host.a`).
- Include `libhost.a` at root as well for backward compatibility.
- Ensure deterministic GNU tar headers (`mtime = 0`, `mode = 0o644` for files, `0o755` for directories, `uid = 0`, `gid = 0`) and alphabetical file sorting.

### 2.3 Consumer Smoke Test (`tooling/test_consumer.sh`)
Add an automated verification script:
```bash
#!/usr/bin/env bash
set -euo pipefail
# 1. Unpack dist/roc-golem-v1.0.0.tar.zst to /tmp/test-consumer/platform
# 2. Create /tmp/test-consumer/app/main.roc importing pf from /tmp/test-consumer/platform
# 3. Run roc check
# 4. Run roc build --target wasm32
# 5. Run wasm-tools component new
# 6. Run wasm-tools validate
```
This script will be added to `.github/workflows/ci.yaml` and `.github/workflows/release.yaml`.

### 2.4 Server Port & Deployment Resiliency (`KI-24`)
In `tooling/test_runtime.sh`:
- Check if `http://127.0.0.1:${PORT}/v1/components` is already responding.
- If running: log that existing server is used; do not attempt a conflicting `golem server run` that fails with `Address already in use`.
- When deploying components:
  - Query `GET http://127.0.0.1:${PORT}/v1/components?component-name=<name>`
  - If already deployed, use the existing `componentId` or update it via `PUT /v1/components/<id>`.
  - If new, `POST /v1/components`.
- Use timestamped worker names (e.g. `counter-test-${TIMESTAMP}`) so that successive runs do not collide with existing worker instances.

---

## 3. Implementation Steps

1. **Modify `tooling/pack_platform/src/main.rs`**:
   - Implement recursive directory traversal for `platform_dir`.
   - Preserve `targets/wasm32/libroc_golem_host.a`.
   - Recompile `pack_platform`.
2. **Implement `tooling/test_consumer.sh`**:
   - Create standalone script that unpacks `.tar.zst` and builds a consumer Roc agent.
   - Run and verify it passes.
3. **Fix `platform/main.roc`**:
   - Remove redundant `import Json` and ensure `roc check platform/main.roc` passes with 0 warnings.
4. **Update `tooling/build.sh`**:
   - Ensure `build_app` outputs both `${app}.wasm` and `${app}_agent.wasm`.
   - Remove `|| true` on `roc fmt`.
   - Remove redundant `build_platform` calls.
5. **Update `golem.yaml` and `README.md`**:
   - Point `streaming-agent` in `golem.yaml` to `build/streaming_agent_agent.wasm`.
   - Update README deploy commands to Golem 1.2.5 syntax.
   - Remove references to `host.wasm`.
6. **Update `tooling/test_runtime.sh`**:
   - Add server health detection, component update/reuse logic, and unique worker naming.
7. **Update Checklist and Progress Tracker**:
   - Reconcile rows 15 & 16 in `docs/feature-checklist.md` with explicit verification tags.
   - Update `docs/progress-tracker.md` with Unit 15 details.
8. **Re-package, Verify and Publish**:
   - Run `./tooling/build.sh all`.
   - Run `./tooling/test_consumer.sh`.
   - Run `./tooling/test_runtime.sh`.
   - Update `README.md` with new BLAKE3 base64url checksum.
   - Upload new assets to GitHub release `v1.0.0` with `gh release upload --clobber`.
   - Sync git and tag `v1.0.0`.

---

## 4. Verification Criteria

- [x] `tar --zstd -tvf dist/roc-golem-v1.0.0.tar.zst` contains `targets/wasm32/libroc_golem_host.a`.
- [x] `tooling/test_consumer.sh` passes cleanly (external consumer builds and validates component).
- [x] `roc check platform/main.roc` exits with 0 errors (warnings handled via code 2).
- [x] `tooling/test_runtime.sh` passes 100% on local Golem server.
- [x] `golem app diagnose` or validation reports valid manifest in `golem.yaml`.
- [x] `curl -fsSL https://github.com/thesparq/roc-golem/releases/download/v1.0.0/<hash>.tar.zst` returns 200 OK.
- [x] `docs/feature-checklist.md` rows 15 and 16 updated with accurate verification status.
- [x] Git repository clean, pushed to `main`, and tagged `v1.0.0`.
