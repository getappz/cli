=== Appz Static Site Generator ===
Contributors: appzdev
Tags: static site, wp-cli, simply static, static export, cli
Requires at least: 6.5
Tested up to: 6.9
Requires PHP: 7.4
Stable tag: 1.0.0
License: GPLv2 or later
License URI: https://www.gnu.org/licenses/gpl-2.0.html

WP-CLI commands for Simply Static — run static site generation from the command line.

== Description ==

Appz Static Site Generator adds WP-CLI commands to the free version of [Simply Static](https://wordpress.org/plugins/simply-static/), letting you run static site exports from the command line.

This is useful for:

* **CI/CD pipelines** — trigger exports as part of your deployment workflow
* **Cron jobs** — schedule exports with system cron instead of WP-Cron
* **Automation** — script exports alongside other build steps
* **Reliability** — synchronous execution means no WP-Cron timeouts or missed batches

= Commands =

* `wp appz build` — Run a full static site export synchronously
* `wp appz build --output-dir=/var/www/static` — Export to a specific directory
* `wp appz status` — Show current export status (running, paused, complete)
* `wp appz cancel` — Cancel a running export

= How It Works =

The plugin runs Simply Static's export tasks sequentially in a single process, bypassing WP-Cron. Each task (setup, URL discovery, fetching, file transfer, wrapup) runs to completion before the next starts, with progress logged to your terminal.

= Requirements =

* [Simply Static](https://wordpress.org/plugins/simply-static/) (free version) must be installed and activated
* [WP-CLI](https://wp-cli.org/) must be available on your server

== Installation ==

1. Install and activate [Simply Static](https://wordpress.org/plugins/simply-static/)
2. Upload the `appz-static-site-generator` folder to `/wp-content/plugins/`
3. Activate the plugin through the 'Plugins' menu in WordPress
4. Run `wp appz build` from your terminal

== Frequently Asked Questions ==

= Does this work with Simply Static Pro? =

This plugin is designed for the free version of Simply Static. The Pro version already includes its own CLI commands.

= Do I need WP-CLI? =

Yes. The CLI commands require [WP-CLI](https://wp-cli.org/). The plugin also provides a minimal info page under Tools → Appz Static Generator that works without WP-CLI.

= What export types are supported? =

The free version of Simply Static supports full site exports. The plugin runs whatever delivery method you have configured in Simply Static settings (ZIP download or local directory).

= Can I override the output directory? =

Yes. Use `--output-dir` to export to a specific directory for this run without changing your Simply Static settings:

`wp appz build --output-dir=/var/www/static`

= Can I use this in a CI/CD pipeline? =

Yes — that's a primary use case. The `wp appz build` command runs synchronously and exits with a non-zero status on failure, making it suitable for scripts and pipelines.

= Does it work with multisite? =

Yes. Use the `--blog-id` flag: `wp appz build --blog-id=2`

== Changelog ==

= 1.0.0 =
* Initial release
* `wp appz build` — synchronous full site export with optional `--output-dir`
* `wp appz status` — export status display
* `wp appz cancel` — cancel running export
* Admin info page under Tools showing backend status and CLI reference

== Upgrade Notice ==

= 1.0.0 =
Initial release.
