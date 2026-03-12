ALTER TABLE `crawl_jobs` ADD `type` text DEFAULT 'crawl' NOT NULL;--> statement-breakpoint
CREATE INDEX `idx_crawl_jobs_type` ON `crawl_jobs` (`type`);