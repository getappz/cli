CREATE TABLE `scrape_cache` (
	`id` text PRIMARY KEY NOT NULL,
	`url_hash` text NOT NULL,
	`cache_key` text NOT NULL,
	`url_resolved` text,
	`created_at_ms` integer NOT NULL,
	`expires_at_ms` integer NOT NULL,
	`schema_version` integer DEFAULT 1 NOT NULL,
	`status_code` integer NOT NULL,
	`r2_key` text NOT NULL,
	`formats` text
);
--> statement-breakpoint
CREATE INDEX `idx_scrape_cache_lookup` ON `scrape_cache` (`url_hash`,`cache_key`,`expires_at_ms`);