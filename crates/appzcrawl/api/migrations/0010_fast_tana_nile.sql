PRAGMA foreign_keys=OFF;--> statement-breakpoint
CREATE TABLE `__new_api_keys` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`key_hash` text NOT NULL,
	`key_prefix` text NOT NULL,
	`team_id` text NOT NULL,
	`name` text DEFAULT 'Default',
	`scopes` text,
	`last_used_at` text,
	`expires_at` text,
	`created_by` text,
	`created_at` text NOT NULL,
	`deleted_at` text
);
--> statement-breakpoint
INSERT INTO `__new_api_keys`("id", "key_hash", "key_prefix", "team_id", "name", "scopes", "last_used_at", "expires_at", "created_by", "created_at", "deleted_at") SELECT "id", "key_hash", "key_prefix", "team_id", "name", "scopes", "last_used_at", "expires_at", "created_by", "created_at", "deleted_at" FROM `api_keys`;--> statement-breakpoint
DROP TABLE `api_keys`;--> statement-breakpoint
ALTER TABLE `__new_api_keys` RENAME TO `api_keys`;--> statement-breakpoint
PRAGMA foreign_keys=ON;--> statement-breakpoint
CREATE UNIQUE INDEX `api_keys_key_hash_unique` ON `api_keys` (`key_hash`);--> statement-breakpoint
CREATE INDEX `idx_api_keys_team` ON `api_keys` (`team_id`);