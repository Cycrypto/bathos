<!-- Thanks for contributing to BATHOS! Keep PRs focused and small. -->

## What & why
Brief description of the change and the issue it addresses. Closes #___

## Scale
- [ ] Lv0 (trivial) · [ ] Lv1 (small) · [ ] Lv2 (standard) · [ ] Lv3+ (large)

## Type
- [ ] Engine (Rust crate) · [ ] Hook / settings · [ ] Slash command · [ ] Role / asset · [ ] Plugin module · [ ] Docs

## Checklist
- [ ] `cargo fmt --all -- --check` clean
- [ ] `cargo clippy --all-targets -- -D warnings` clean
- [ ] `cargo test --all` green; **added/updated tests** for the change
- [ ] `bash .claude/hooks/_test-hooks.sh` green (if hooks/settings touched)
- [ ] Core stays slim — no new core → plugin dependency (A9)
- [ ] Docs updated (`README.md` / `docs/USAGE-kr.md`) and `CHANGELOG.md` *Unreleased* entry added
- [ ] Gate vocabulary (PASS/CONCERNS/FAIL) and safety hooks not weakened

## Notes for reviewers
Anything that needs special attention, trade-offs, or follow-ups.
