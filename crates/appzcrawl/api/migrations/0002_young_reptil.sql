CREATE TABLE `crawl_jobs` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`origin_url` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`crawler_options` text,
	`scrape_options` text,
	`robots_txt` text,
	`completed_count` integer DEFAULT 0 NOT NULL,
	`total_count` integer DEFAULT 0 NOT NULL,
	`credits_billed` integer DEFAULT 0 NOT NULL,
	`webhook` text,
	`cancelled` integer DEFAULT false NOT NULL,
	`zero_data_retention` integer DEFAULT false NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	`expires_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_crawl_jobs_team_status` ON `crawl_jobs` (`team_id`,`status`);--> statement-breakpoint
CREATE TABLE `crawl_results` (
	`id` text PRIMARY KEY NOT NULL,
	`crawl_id` text NOT NULL,
	`url` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`r2_key` text,
	`document_json` text,
	`error` text,
	`status_code` integer,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_crawl_results_crawl` ON `crawl_results` (`crawl_id`,`status`);