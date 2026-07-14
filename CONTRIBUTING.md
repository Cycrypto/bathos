# Contributing to BATHOS

Thanks for your interest in BATHOS! BATHOS *is* a development method, so the best way to contribute is to **use it on itself**. 🇰🇷 한국어 안내는 각 절 하단에 함께 둡니다.

## Ground rules

1. **Open an issue first** for anything non-trivial — describe the change and its scale (Lv0–4, see the README).
2. **Keep the core slim.** New domain capabilities belong in `modules/` as plugins, never in `core/`. The core must not depend on any plugin (the A9 rule).
3. **Respect the gate vocabulary** — `PASS` / `CONCERNS` / `FAIL` — and the safety hooks (`careful`, `freeze`). Don’t weaken them to make CI pass.
4. **Generation ≠ verification.** If you generated code, get it independently reviewed before claiming it’s done.

> 🇰🇷 사소하지 않은 변경은 먼저 이슈로 등록하고 작업 규모(Lv0~4)를 명시하세요. 도메인 기능은 `core/`가 아니라 `modules/` 플러그인으로. 게이트 용어·안전 훅을 약화시키지 마세요.

## Development setup

```bash
# Build the engine
cd core
cargo build --release

# The full local check (must be green before you push):
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all

# Hook determinism harness (needs jq + a built binary)
cd ..
BATHOS_BIN="$(pwd)/core/target/release/bathos" bash .claude/hooks/_test-hooks.sh
```

CI (`.github/workflows/ci.yml`) runs exactly these checks on every push and pull request.

## Pull requests

- Branch from `main`; keep PRs focused and small.
- **Add or update tests** for every behavior change. Engine invariants (audit chain, gates, concurrency, story freshness) must stay covered.
- Update docs when behavior changes: `README.md`, `docs/USAGE-kr.md`, and `CHANGELOG.md` (add an entry under *Unreleased*).
- Match the surrounding code: comment density, naming, and idiom. Engine comments and docs are written in Korean with English identifiers — follow the file you’re editing.
- PRs must pass CI (build · fmt · clippy · test · hook harness).

> 🇰🇷 동작이 바뀌면 반드시 테스트를 추가/갱신하고, `README.md`·`docs/USAGE-kr.md`·`CHANGELOG.md`(Unreleased)를 갱신하세요. PR은 CI(빌드·fmt·clippy·test·훅 하네스) 통과 필수.

## Reporting bugs & requesting features

Use the issue templates under `.github/ISSUE_TEMPLATE/`. For security-sensitive reports, please do **not** open a public issue — see `SECURITY` contact in `CODE_OF_CONDUCT.md`.

## Operating principles (`ETHOS.md`)

- **User Sovereignty** — AI proposes, the human decides.
- **Boil the Ocean** — if finishing completely is only minutes more, finish.
- **Search Before Building** — survey the terrain before writing code.

By contributing, you agree that your contributions are licensed under the project’s [MIT License](LICENSE).
