#!/usr/bin/env node

const { spawnSync, execSync } = require("node:child_process");
const { existsSync, readdirSync } = require("node:fs");
const path = require("node:path");

const PLATFORMS = {
	"linux-x64-glibc": "@getappz/cli-linux-x64",
	"linux-x64-musl": "@getappz/cli-linux-x64-musl",
	"linux-arm64-glibc": "@getappz/cli-linux-arm64",
	"linux-arm64-musl": "@getappz/cli-linux-arm64-musl",
	"darwin-x64": "@getappz/cli-darwin-x64",
	"darwin-arm64": "@getappz/cli-darwin-arm64",
	"win32-x64": "@getappz/cli-win32-x64",
};

function detectLibc() {
	if (process.platform !== "linux") return null;

	try {
		const libs = readdirSync("/lib");
		if (libs.some((f) => f.startsWith("ld-musl-"))) return "musl";
	} catch {}

	try {
		const lddOutput = execSync("ldd --version 2>&1", { encoding: "utf8" });
		if (lddOutput.toLowerCase().includes("musl")) return "musl";
	} catch {}

	return "glibc";
}

function getPlatformPackage() {
	const { platform, arch } = process;

	const key =
		platform === "linux"
			? `${platform}-${arch}-${detectLibc()}`
			: `${platform}-${arch}`;

	const pkg = PLATFORMS[key];
	if (!pkg) {
		console.error(
			`Error: Unsupported platform "${key}".\n\n` +
				"Appz CLI supports:\n" +
				Object.entries(PLATFORMS)
					.map(([k, v]) => `  - ${k} (${v})`)
					.join("\n") +
				"\n\nIf you believe this platform should be supported, please file an issue.",
		);
		process.exit(1);
	}

	return pkg;
}

function getBinaryPath(pkg) {
	const binaryName = process.platform === "win32" ? "appz.exe" : "appz";
	try {
		const pkgDir = path.dirname(require.resolve(`${pkg}/package.json`));
		return path.join(pkgDir, "bin", binaryName);
	} catch {
		console.error(
			`Error: Platform package "${pkg}" is not installed.\n\n` +
				"Try reinstalling:\n" +
				"  npm install @getappz/cli\n",
		);
		process.exit(1);
	}
}

const pkg = getPlatformPackage();
const binaryPath = getBinaryPath(pkg);

if (!existsSync(binaryPath)) {
	console.error(
		`Error: Binary not found at "${binaryPath}".\n\n` +
			`The platform package "${pkg}" is installed but the binary is missing.\n` +
			"Try reinstalling:\n" +
			"  npm install @getappz/cli\n",
	);
	process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
	stdio: "inherit",
});

if (result.error) {
	console.error(
		`Error: Failed to execute appz binary: ${result.error.message}`,
	);
	process.exit(1);
}

if (result.signal) {
	process.kill(process.pid, result.signal);
} else {
	process.exit(result.status ?? 1);
}
