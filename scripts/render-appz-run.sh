#!/usr/bin/env bash
# shellcheck disable=SC2016
set -euxo pipefail

BASE_DIR="$(pwd)"

# Create the appz.run directory in releases
mkdir -p "artifacts/appz.run"

# Function to generate shell-specific script
generate_shell_script() {
	local shell_name="$1"
	local final_message="$2"

	echo "Generating $shell_name script..."

	# Export variables for envsubst
	export SHELL_NAME="$shell_name"
	export FINAL_MESSAGE="$final_message"

	# Generate script from template
	envsubst '$SHELL_NAME,$FINAL_MESSAGE' <"$BASE_DIR/packaging/appz.run/shell.envsubst" >"artifacts/appz.run/$shell_name"

	# Make executable
	chmod +x "artifacts/appz.run/$shell_name"

	# Validate with shellcheck if available
	if command -v shellcheck >/dev/null 2>&1; then
		shellcheck "artifacts/appz.run/$shell_name" || true
	fi
}

# Generate bash script
generate_shell_script "bash" \
	"restart your shell or run 'source ~/.bashrc' to start using appz"

# Generate zsh script
generate_shell_script "zsh" \
	"restart your shell or run 'source \${ZDOTDIR-\$HOME}/.zshrc' to start using appz"

# Generate fish script
generate_shell_script "fish" \
	"restart your fish shell or run 'source ~/.config/fish/config.fish' to start using appz"

echo "Shell scripts generated successfully in artifacts/appz.run/"

