#!/bin/sh
set -eu

# Extract version from Cargo.toml
VERSION=$(grep '^version =' Cargo.toml | head -n1 | cut -d '"' -f 2)

# If we're in a git tag context, use the tag name
if [ -n "${GITHUB_REF:-}" ] && echo "$GITHUB_REF" | grep -q '^refs/tags/v'; then
    TAG=$(echo "$GITHUB_REF" | sed 's/^refs\/tags\///')
    echo "$TAG"
else
    echo "v$VERSION"
fi

