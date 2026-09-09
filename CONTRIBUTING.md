# Publishing a new version

This hybrid Python-Rust package is published on PyPI.

1. Bump the version number in `Cargo.toml`. Add it to a new commit.
2. Tag that commit with the new version number prefixed with `v`. For example, `git tag v0.1.0`.
3. Push that commit to the `main` branch, going through a PR if necessary. Push the tag as well: `git push origin main --tags`.
4. The CI will publish a new GitHub release and a new version on PyPI.
