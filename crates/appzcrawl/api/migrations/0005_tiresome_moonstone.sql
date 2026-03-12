CREATE TABLE `scrapes` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`url` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`success` integer DEFAULT false NOT NULL,
	`options` text,
	`result` text,
	`r2_key` text,
	`error` text,
	`zero_data_retention` integer DEFAULT false NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_scrapes_team` ON `scrapes` (`team_id`);--> statement-breakpoint
CREATE INDEX `idx_scrapes_status` ON `scrapes` (`status`);