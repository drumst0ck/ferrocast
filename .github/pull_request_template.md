## What and why

## Checks
- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` pass (or the change is web-only and `cd web && npm run build && npm test` pass)
- [ ] `CHANGELOG.md` has an entry under `[Unreleased]`
- [ ] A docs page was added or updated if behavior changed
- [ ] Nothing is marked Tested unless I ran it (real files or a real session)
- [ ] Anything that needs Accessibility or a live session is described below, with how I checked it
