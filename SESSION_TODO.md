# Session TODO List

## 1. Restore Terminal Safety
- [x] **Read Cargo.toml**: Confirm current `panic` setting.
- [x] **Update Cargo.toml**: Change `panic = "abort"` to `panic = "unwind"` (or remove it) in `[profile.release]` to ensure `Drop` handlers (RAII) run on panic.

## 2. Fix Broken Build (ftui-widgets)
- [x] **Create block.rs**: Create `crates/ftui-widgets/src/block.rs` as an empty placeholder to satisfy `mod block;`.
- [x] **Create paragraph.rs**: Create `crates/ftui-widgets/src/paragraph.rs` as an empty placeholder to satisfy `mod paragraph;`.

## 3. Verification & Quality Gates
- [ ] **Compile**: Run `cargo check --workspace` to ensure the build is unblocked.
- [ ] **Lint**: Run `cargo clippy --workspace -- -D warnings` to catch common errors.
- [ ] **Format**: Run `cargo fmt --check` to ensure code style compliance.

## 4. Deep Analysis (UBS)
- [ ] **Run UBS**: Run `ubs .` to scan for deeper bugs.
- [ ] **Triage**: Analyze any findings from UBS.
- [ ] **Fix**: Apply fixes for confirmed bugs (strictly no new features).
