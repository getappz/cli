#!/usr/bin/env bash
# Description: Prepare release by bumping version and generating changelog
set -euxo pipefail

# Ensure this script is only run in GitHub Actions
if [[ -z ${GITHUB_ACTIONS:-} ]]; then
	echo "Error: This script must be run in GitHub Actions"
	echo "The release-prepare script should only be executed in the CI/CD pipeline"
	exit 1
fi

git config user.name appz-release-bot
git config user.email release@example.com

# Get version from cli crate (the binary crate)
cur_version="$(grep '^version =' crates/cli/Cargo.toml | head -n1 | cut -d '"' -f 2)"

# Check if tag already exists (release already published)
if git rev-parse -q --verify "refs/tags/v$cur_version" >/dev/null ||
   git ls-remote --exit-code --tags origin "v$cur_version" >/dev/null 2>&1; then
	echo "Tag v$cur_version already exists, skipping release preparation"
	exit 0
fi

# Check if there's already a release PR open
if [[ "$(gh pr list --label release --head release 2>/dev/null || echo "")" != "" ]]; then
	echo "Release PR already exists, skipping"
	exit 0
fi

# Bump version based on date (year.month.patch format like mise)
year="$(date +%Y)"
month="$(date +%-m)"
if echo "$cur_version" | grep -qE "^$year\.$month\."; then
	cargo set-version --bump patch -p cli
elif echo "$cur_version" | grep -qE "^$year\."; then
	cargo set-version --bump minor -p cli
else
	cargo set-version "$year.1.0" -p cli
fi

# Get new version
version="$(grep '^version =' crates/cli/Cargo.toml | head -n1 | cut -d '"' -f 2)"

# Update version in other files
if [[ -f snapcraft.yaml ]]; then
	sed -i.bak "s/version: \"[0-9]\+\.[0-9]\+\.[0-9]\+\(-rc\.[0-9]\+\)\?\"$/version: \"$version\"/" snapcraft.yaml
	rm -f snapcraft.yaml.bak
fi

if [[ -f winget/appz.yaml ]]; then
	sed -i.bak "s/^PackageVersion: .*$/PackageVersion: $version/" winget/appz.yaml
	rm -f winget/appz.yaml.bak
fi

if [[ -f chocolatey/appz/appz.nuspec ]]; then
	sed -i.bak "s/<version>[0-9]\+\.[0-9]\+\.[0-9]\+\(-rc\.[0-9]\+\)\?<\/version>/<version>$version<\/version>/" chocolatey/appz/appz.nuspec
	rm -f chocolatey/appz/appz.nuspec.bak
fi

# Generate changelog for new release section
# Use --prepend to preserve existing changelog content
git cliff --tag "v$version" --unreleased --prepend CHANGELOG.md || {
	echo "Warning: git-cliff failed, creating empty changelog entry"
	if [[ ! -f CHANGELOG.md ]]; then
		echo "# Changelog" > CHANGELOG.md
	fi
	cat >> CHANGELOG.md <<EOF

## [$version] - $(date +%Y-%m-%d)

### Changes
- See git log for details

EOF
}

# Generate changelog for PR body
changelog="$(git cliff --tag "v$version" --unreleased --strip all || echo "See CHANGELOG.md for details")"
changelog="$(echo "$changelog" | tail -n +3)"

# Stage changes
git add \
	Cargo.lock \
	Cargo.toml \
	CHANGELOG.md \
	snapcraft.yaml \
	winget/appz.yaml \
	chocolatey/appz/appz.nuspec \
	2>/dev/null || true

git clean -df

# Create release branch
git checkout -B release
git commit -m "chore: release $version" || {
	echo "No changes to commit"
	exit 0
}

git push origin release --force

# Create or update PR
if [[ "$(gh pr list --label release --head release 2>/dev/null || echo "")" == "" ]]; then
	gh pr create --title "chore: release $version" --body "$changelog" --label "release" --head release || {
		echo "Failed to create PR"
		exit 1
	}
else
	gh pr edit --title "chore: release $version" --body "$changelog" || {
		echo "Failed to update PR"
		exit 1
	}
fi

