# Release Setup Guide

This guide explains how to configure the CI/CD pipeline to publish releases to a public GitHub repository while keeping your source code private.

## Overview

The release workflow supports two scenarios:
1. **Private repo with public releases**: Releases are published to the same repository (if releases are made public)
2. **Private repo with separate public releases repo**: Releases are published to a separate public repository

## Configuration

### Option 1: Same Repository (Public Releases)

If you want to publish releases to the same repository but make them public:

1. Ensure your repository allows public releases (even if the repo is private)
2. No additional configuration needed - the workflow will use `github.repository` by default

### Option 2: Separate Public Repository

If you want to publish releases to a separate public repository:

1. Create a public GitHub repository (e.g., `yourusername/appz-releases`)
2. Add the following secrets to your private repository:
   - `PUBLIC_REPO`: The full repository name (e.g., `yourusername/appz-releases`)
   - `PUBLIC_REPO_TOKEN`: A GitHub Personal Access Token with `repo` scope for the public repository

#### Creating a GitHub Personal Access Token

1. Go to GitHub Settings → Developer settings → Personal access tokens → Tokens (classic)
2. Generate a new token with `repo` scope
3. Add it as a secret named `PUBLIC_REPO_TOKEN` in your private repository

## Package Manager Configuration

### Winget

1. Fork or create a repository at `yourusername/winget-pkgs`
2. Update `.github/workflows/winget.yml` with your repository name
3. The workflow will automatically update the winget manifest when releases are published

### Chocolatey

1. Create a Chocolatey package using the files in `chocolatey/appz/`
2. Update the URLs in `chocolatey/appz/tools/chocolateyInstall.ps1` to point to your public releases
3. Submit to Chocolatey Community Repository or host your own feed

### Snapcraft

1. Register at [Snapcraft](https://snapcraft.io/)
2. Add `SNAPCRAFT_STORE_CREDENTIALS` secret to your repository
3. Update `snapcraft.yaml` with your repository information

### PPA (Ubuntu/Debian)

1. Create a Launchpad account and PPA
2. Add GPG key as `GPG_KEY` secret
3. Update `PPA_NAME` in repository variables or workflow

### COPR (Fedora/RHEL)

1. Create a COPR account
2. Add `COPR_API_LOGIN` and `COPR_API_TOKEN` secrets
3. Update COPR owner/project in repository variables

### npm

1. Create an npm account
2. Add `NPM_TOKEN` secret
3. Update package name in `.github/workflows/npm-publish.yml` if needed

## Testing

To test the release workflow:

1. Create a test tag: `git tag v0.1.0-test`
2. Push the tag: `git push origin v0.1.0-test`
3. The workflow will build and attempt to create a release
4. Check the Actions tab for any errors

## Troubleshooting

### Release creation fails

- Verify `PUBLIC_REPO_TOKEN` has correct permissions
- Check that the public repository exists and is accessible
- Ensure the tag format matches `v*` pattern

### Artifacts not found

- Verify build jobs completed successfully
- Check artifact names match the expected patterns
- Ensure build scripts are executable (Linux/macOS)

### Package manager workflows fail

- Verify all required secrets are configured
- Check that package manager accounts are set up correctly
- Review workflow logs for specific error messages

