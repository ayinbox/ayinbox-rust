# Publishing the crate

The crate is published to crates.io by `.github/workflows/release.yml` when a `v*` tag is pushed. It uses crates.io trusted publishing (OIDC through `rust-lang/crates-io-auth-action`): no API token is stored in GitHub.

crates.io only lets you configure trusted publishing on a crate that already exists, so the very first release is published by hand.

## One-time setup

1. Sign in to https://crates.io with the GitHub account that will own `ayinbox`, and verify an email address under Account Settings (crates.io refuses to publish without one).
2. First release, by hand:
   1. On https://crates.io/settings/tokens create a token with the `publish-new` scope, limited to the crate name `ayinbox`, expiring in a day or less.
   2. From a clean checkout of `main` at the commit you mean to release:

      ```sh
      cargo login            # paste the token; it is stored in ~/.cargo/credentials.toml
      cargo publish --dry-run
      cargo publish
      cargo logout           # removes the stored token
      ```

   3. Revoke the token on https://crates.io/settings/tokens.
   4. Tag that commit so history matches the registry: `git tag v0.1.0 && git push origin v0.1.0`. The release workflow will run and fail at `cargo publish` because 0.1.0 already exists; that failure is expected for this one tag.
3. Configure trusted publishing: open https://crates.io/crates/ayinbox/settings, under "Trusted Publishing" add a GitHub publisher with:
   - Repository owner: `ayinbox`
   - Repository name: `ayinbox-rust`
   - Workflow filename: `release.yml`
   - Environment: `release`
4. In GitHub, open `ayinbox/ayinbox-rust` > Settings > Environments, create an environment named `release`, and (optionally) add required reviewers so a tag cannot publish without approval.
5. Optionally, in the crate settings turn on "require trusted publishing" so tokens can no longer publish it, and add co-owners with `cargo owner --add <github-user>`.

## Releasing

1. Update `version` in `Cargo.toml` and `CHANGELOG.md`, commit to `main`, and wait for CI to pass.
2. Tag and push:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

The workflow checks that the tag matches the crate version, runs the tests, exchanges the GitHub OIDC token for a short-lived crates.io token and runs `cargo publish`.

After the first publish, change the README install section from the git dependency to `ayinbox = "0.1"` (or `cargo add ayinbox`).
