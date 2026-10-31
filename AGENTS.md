# Agent Guidelines (AGENTS.md)

If you are an AI assistant (like Claude Code or Antigravity) working in this repository, you must read and follow these rules.

## 1. Version Control via Jujutsu (`jj`)

The user uses **Jujutsu (`jj`)** for version control.

- **Do NOT use `git` commands** (e.g., `git commit`, `git add`, `git checkout`).
- Use `jj` for repository operations. Every Jujutsu command displaying output (specifically `log`, `status`, `diff`, and `bookmark` commands) **MUST** use the `--no-pager` global option to prevent terminal interactive hangs (e.g., `jj --no-pager status`, `jj --no-pager diff`, `jj --no-pager log`).
- Jujutsu automatically tracks file changes as you write them. When you want to set a commit message for the active change, use:
  ```bash
  jj --no-pager describe -m "Your descriptive commit message"
  ```
- If you need to start a new logical change, use `jj new`.

## 2. Comprehensive Quality Checks

Before submitting changes or opening pull requests, review agents must ensure that all automated checks pass cleanly:

- **Rust formatting**: `cargo fmt --check`
- **Rust compiler & Clippy lints**: `cargo clippy --all-targets --all-features -- -D warnings`
- **Purist AST linter**: `purist`
- **API manifest check**: `api-manifest check`
- **Dependency security audit**: `cargo audit`
- **TOML formatting**: `taplo fmt --check`
- **Markdown & JSON formatting**: `prettier --check "**/*.{md,json}"`

## 3. Purist AST Linter

Review agents must always run the Purist AST linter (`purist`) to identify and resolve architectural, style, and hygiene issues:

```bash
purist
```

Ensure zero violations (errors or warnings) are reported before submitting changes.

## 4. Testing & Verification

- **Unit & Integration Tests**: Keep all tests passing:
  ```bash
  cargo test --all-targets --all-features
  ```
- **Rust Formatting**: Format Rust code with `cargo fmt --check` (or `cargo fmt` to apply).
- **Clippy**: Ensure zero Clippy warnings (`cargo clippy --all-targets --all-features -- -D warnings`).
- **API Manifest**: Keep `API.md` synchronized with the public library and CLI API (`api-manifest check`, or `api-manifest dump && prettier --write API.md` to regenerate).
- **TOML Formatting**: Ensure all `.toml` files are formatted using Taplo (`taplo fmt --check` or `taplo fmt`).
- **Markdown & JSON Formatting**: Ensure all `.md` and `.json` files match Prettier style (`prettier --check "**/*.{md,json}"`).
- **Dependency Audit**: Ensure no known security advisories exist (`cargo audit`).
- **Nix Flake & Build**: In environments with Nix, verify that the flake checks and builds cleanly:
  ```bash
  nix flake check && nix build
  ```

## 5. Coding & Cleanliness

- Do not introduce unnecessary dependencies. Keep the codebase lightweight.
