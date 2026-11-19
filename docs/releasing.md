# Releasing appz

Every push to the `release` branch runs `.github/workflows/release.yml`. It builds the binaries,
writes `SHA256SUMS` (also published as `SHA256SUMS.txt`), signs the archives with cosign (keyless) and creates or updates a **draft**
GitHub Release `v<version>` targeting the pushed commit, with generated notes. Nobody sees a draft
until a maintainer publishes it.

## Cutting a release

1. **Bump the version** on `main`. It lives only in `[workspace.package] version` in the root
   `Cargo.toml`:

   ```sh
   mise run version-set 0.3.1          # or 0.3.1-rc.1; edits Cargo.toml and runs cargo update -w
   ```

   Merge the change through the normal PR flow.
2. **Merge `main` into `release`** (or fast-forward it) and push. The workflow starts by itself.
3. **Check the draft.** When the jobs finish, the Releases page has a draft `v0.3.1` with every
   archive, cosign bundle, the source tarball and `SHA256SUMS.txt`.
4. **Publish** the draft in the GitHub UI. Publishing creates the tag and fires
   `.github/workflows/release-published.yml`: SLSA provenance (hashes of the published archives),
   Homebrew, Scoop, npm and Docker. Pre-release versions (`-rc.1`) are marked as pre-releases and
   only get provenance; the package-manager jobs skip them.

Pushing to `release` again before publishing rebuilds the same draft and replaces its assets. Once
published, the workflow refuses to touch that version; bump it first.

**Test runs:** *Actions > release > Run workflow*. The optional `version` input (e.g. `0.3.1-rc.1`)
overrides `Cargo.toml` for that run only. Choose the `release` branch for a full run; on any other
branch the dispatch is a dry run that builds the matrix and stops (the draft job is skipped).

## One-time repo setup

- Create a `release` environment (Settings > Environments) with a deployment-branch rule limited to
  `release`. The draft job uses it.
- Protect the `release` branch like `main`.

## Version reporting

`appz --version` prints `appz <version> (<sha>)`. CI sets `APPZ_BUILD_SHA`; local builds read
`git rev-parse`.
