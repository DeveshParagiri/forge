# Contributing to Exaforge

Exaforge is a personal fork of [xai-org/grok-build](https://github.com/xai-org/grok-build).
Contributions that improve the Exaforge extension layer are welcome when they
are focused, tested, and easy to carry across upstream updates.

## Scope

- Put substantial fork-specific logic in the appropriate per-crate
  `src/exaforge/` module.
- Keep edits to upstream-owned files small and mark integration points with
  `// Exaforge:` where practical.
- Preserve the stock SpaceXAI login and model paths.
- Do not combine behavior-neutral refactors with unrelated behavior changes.
- Send changes that apply only to stock Grok Build to the upstream project.

## Before submitting

Run formatting and the focused checks for every affected crate:

```sh
cargo fmt --all
cargo check -p <crate>
cargo test -p <crate> <relevant-test-filter>
```

Describe the behavior being changed, the upstream files touched, and the tests
run. Never include credentials, tokens, private configuration, or session data.

## Security

Follow [`SECURITY.md`](SECURITY.md) for vulnerability reports. Do not disclose
security issues in a public issue or pull request.

## License

Contributions are accepted under the repository's Apache License 2.0 terms.
