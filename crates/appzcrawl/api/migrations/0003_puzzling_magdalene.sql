CREATE TABLE `crawl_robots_blocked` (
	`crawl_id` text NOT NULL,
	`url` text NOT NULL,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_crawl_robots_blocked` ON `crawl_robots_blocked` (`crawl_id`);--> statement-breakpoint
CREATE INDEX `idx_crawl_robots_blocked_unique` ON `crawl_robots_blocked` (`crawl_id`,`url`);--> statement-breakpoint
ALTER TABLE `crawl_results` ADD `code` text;