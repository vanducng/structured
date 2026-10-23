# Deployment

`structured` ships as GitHub release archives. Nothing runs on a server.

## CI

`.github/workflows/ci.yml` runs on every pull request and on pushes to `main`:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Releases

Releases are automatic and driven by conventional commits.

1. Commits land on `main`. release-please (`.release-please-config.json`, `.release-please-manifest.json`) keeps a release PR open with the next version and its `CHANGELOG.md` entry.
2. `fix` bumps the patch. `feat` and breaking changes bump the minor while the version is below 1.0. `docs`, `refactor`, `chore`, `test`, and `ci` alone cut no release.
3. Merging the release PR updates `Cargo.toml` and `Cargo.lock`, tags `vX.Y.Z`, and creates the GitHub release.
4. The same `release` workflow run builds both targets with `cargo build --release --locked`, checks that `structured --version` matches the tag, packs `structured`, `README.md`, and `LICENSE` into `structured-vX.Y.Z-TARGET.tar.gz`, and uploads the archives with `SHA256SUMS`.

Do not edit `CHANGELOG.md` or the version by hand. To force a version, push an empty commit to `main` with a `Release-As: X.Y.Z` footer.

The build and release-please share one workflow run because a tag created with `GITHUB_TOKEN` starts no new run.

## Rebuild assets

Run the `release` workflow by hand with an existing tag, like `v0.2.0`, to rebuild that tag's archives and upload them again:

```bash
gh workflow run release --repo vanducng/structured -f tag=v0.2.0
```

## Install or upgrade

The [README](../README.md#install) has the download and checksum steps. Upgrading is the same steps with the newer tag.

## Roll back

Install the previous release's archive the same way, naming its tag instead of the latest. To pull a bad release, mark it as a pre-release or delete it in GitHub, then ship the fix as a new `fix` commit so release-please cuts the next patch.

Signing in again is never part of a rollback: `auth.json` and `config.toml` stay where they are across versions.
