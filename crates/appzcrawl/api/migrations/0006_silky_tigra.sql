CREATE TABLE `agent_jobs` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`prompt` text NOT NULL,
	`urls` text,
	`schema_json` text,
	`model` text,
	`options` text,
	`result` text,
	`r2_key` text,
	`error` text,
	`credits_billed` integer DEFAULT 0 NOT NULL,
	`webhook` text,
	`zero_data_retention` integer DEFAULT false NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	`expires_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_agent_jobs_team` ON `agent_jobs` (`team_id`);--> statement-breakpoint
CREATE INDEX `idx_agent_jobs_status` ON `agent_jobs` (`status`);--> statement-breakpoint
CREATE TABLE `extract_jobs` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`urls` text,
	`prompt` text,
	`schema_json` text,
	`system_prompt` text,
	`options` text,
	`result` text,
	`r2_key` text,
	`error` text,
	`warning` text,
	`credits_billed` integer DEFAULT 0 NOT NULL,
	`webhook` text,
	`zero_data_retention` integer DEFAULT false NOT NULL,
	`created_at` text NOT NULL,
	`updated_at` text NOT NULL,
	`expires_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_extract_jobs_team` ON `extract_jobs` (`team_id`);--> statement-breakpoint
CREATE INDEX `idx_extract_jobs_status` ON `extract_jobs` (`status`);