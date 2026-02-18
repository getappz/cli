CREATE TABLE `crawl_visited_urls` (
	`crawl_id` text NOT NULL,
	`url_hash` text NOT NULL,
	`url` text NOT NULL,
	`created_at` text NOT NULL,
	PRIMARY KEY(`crawl_id`, `url_hash`)
);
--> statement-breakpoint
CREATE TABLE `teams` (
	`id` text PRIMARY KEY NOT NULL,
	`name` text,
	`hmac_secret` text,
	`rate_limit_scrape` integer DEFAULT 100,
	`rate_limit_crawl` integer DEFAULT 15,
	`rate_limit_search` integer DEFAULT 100,
	`rate_limit_extract` integer DEFAULT 100,
	`rate_limit_map` integer DEFAULT 100,
	`max_concurrency` integer DEFAULT 10,
	`crawl_ttl_hours` integer DEFAULT 24,
	`flags` text,
	`auto_recharge` integer DEFAULT false NOT NULL,
	`auto_recharge_threshold` integer,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL
);
--> statement-breakpoint
CREATE TABLE `url_blocklist` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`pattern` text NOT NULL,
	`reason` text,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `url_blocklist_pattern_unique` ON `url_blocklist` (`pattern`);