CREATE TABLE `billing_log` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`job_id` text,
	`job_type` text,
	`credits` integer NOT NULL,
	`reason` text NOT NULL,
	`balance_after` integer,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_billing_log_team` ON `billing_log` (`team_id`);--> statement-breakpoint
CREATE INDEX `idx_billing_log_created` ON `billing_log` (`created_at`);--> statement-breakpoint
CREATE TABLE `request_log` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`api_key_id` integer,
	`endpoint` text NOT NULL,
	`method` text NOT NULL,
	`job_id` text,
	`credits_billed` integer DEFAULT 0 NOT NULL,
	`status_code` integer,
	`duration_ms` integer,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_request_log_team` ON `request_log` (`team_id`);--> statement-breakpoint
CREATE INDEX `idx_request_log_created` ON `request_log` (`created_at`);--> statement-breakpoint
CREATE TABLE `webhook_logs` (
	`id` text PRIMARY KEY NOT NULL,
	`team_id` text NOT NULL,
	`job_id` text NOT NULL,
	`job_type` text NOT NULL,
	`event` text NOT NULL,
	`url` text NOT NULL,
	`status_code` integer,
	`success` integer DEFAULT false NOT NULL,
	`attempt` integer DEFAULT 1 NOT NULL,
	`request_body` text,
	`response_body` text,
	`error` text,
	`created_at` text NOT NULL
);
--> statement-breakpoint
CREATE INDEX `idx_webhook_logs_job` ON `webhook_logs` (`job_id`);--> statement-breakpoint
CREATE INDEX `idx_webhook_logs_team` ON `webhook_logs` (`team_id`);