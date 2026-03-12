/**
 * Crawl completion checker: detects when all URLs in a fan-out crawl have been processed.
 *
 * For high-volume crawls using the SCRAPE_QUEUE fan-out pattern:
 * - crawl-runner discovers URLs and enqueues them to SCRAPE_QUEUE
 * - scrape-consumer processes each URL independently
 * - This checker polls D1 to detect when completed_count >= total_count
 *
 * Can be called:
 * 1. After each scrape-consumer batch completes
 * 2. Via a scheduled cron trigger
 * 3. By the status endpoint when queried
 */

import { logger } from "../lib/logger";
import { getCrawlJob, updateCrawlStatus } from "./crawl-store";

/**
 * Check if a crawl job has completed (all URLs processed).
 * If complete, updates status to "completed" and sends webhook.
 *
 * @returns true if the crawl was just marked as completed
 */
export async function checkCrawlCompletion(
  db: D1Database,
  crawlId: string,
): Promise<boolean> {
  const crawl = await getCrawlJob(db, crawlId);
  if (!crawl) {
    logger.warn("[completion-checker] crawl not found", { crawlId });
    return false;
  }

  // Only check crawls that are in "scraping" status
  if (crawl.status !== "scraping") {
    return false;
  }

  // Check if cancelled
  if (crawl.cancelled) {
    await updateCrawlStatus(db, crawlId, "cancelled");
    return false;
  }

  // Check if all URLs have been processed
  // total_count = number of URLs enqueued
  // completed_count = number of URLs processed by scrape-consumer
  if (crawl.totalCount > 0 && crawl.completedCount >= crawl.totalCount) {
    // All done!
    await updateCrawlStatus(db, crawlId, "completed", {
      completedCount: crawl.completedCount,
      totalCount: crawl.totalCount,
    });

    logger.info("[completion-checker] crawl completed", {
      crawlId,
      completed: crawl.completedCount,
      total: crawl.totalCount,
    });

    // Send webhook if configured
    if (crawl.webhook) {
      try {
        await fetch(crawl.webhook, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({
            success: true,
            type: "crawl.completed",
            id: crawlId,
            data: {
              completed: crawl.completedCount,
              total: crawl.totalCount,
            },
          }),
        });
      } catch (e) {
        logger.warn("[completion-checker] webhook notification failed", {
          crawlId,
          error: e instanceof Error ? e.message : String(e),
        });
      }
    }

    return true;
  }

  return false;
}

/**
 * Check completion for all "scraping" crawls (called by cron).
 * Returns the number of crawls that were marked as completed.
 */
export async function checkAllCrawlCompletions(
  db: D1Database,
): Promise<number> {
  // Find all crawls in "scraping" status
  const { results } = await db
    .prepare(
      `SELECT id FROM crawl_jobs
       WHERE status = 'scraping' AND cancelled = 0
       ORDER BY created_at ASC
       LIMIT 100`,
    )
    .all<{ id: string }>();

  if (!results || results.length === 0) {
    return 0;
  }

  let completedCount = 0;
  for (const row of results) {
    const wasCompleted = await checkCrawlCompletion(db, row.id);
    if (wasCompleted) {
      completedCount++;
    }
  }

  if (completedCount > 0) {
    logger.info("[completion-checker] batch check completed", {
      checked: results.length,
      completed: completedCount,
    });
  }

  return completedCount;
}

/**
 * Get the current progress of a crawl (for status endpoint).
 */
export interface CrawlProgress {
  status: string;
  completed: number;
  total: number;
  /** Percentage complete (0-100) */
  percentage: number;
  /** Estimated time remaining in seconds (null if unknown) */
  etaSeconds: number | null;
}

export async function getCrawlProgress(
  db: D1Database,
  crawlId: string,
): Promise<CrawlProgress | null> {
  const crawl = await getCrawlJob(db, crawlId);
  if (!crawl) return null;

  const percentage =
    crawl.totalCount > 0
      ? Math.round((crawl.completedCount / crawl.totalCount) * 100)
      : 0;

  // Estimate ETA based on elapsed time and progress
  let etaSeconds: number | null = null;
  if (crawl.status === "scraping" && percentage > 0 && percentage < 100) {
    const elapsed = (Date.now() - new Date(crawl.createdAt).getTime()) / 1000;
    const ratePerSecond = crawl.completedCount / elapsed;
    if (ratePerSecond > 0) {
      const remaining = crawl.totalCount - crawl.completedCount;
      etaSeconds = Math.round(remaining / ratePerSecond);
    }
  }

  return {
    status: crawl.status,
    completed: crawl.completedCount,
    total: crawl.totalCount,
    percentage,
    etaSeconds,
  };
}
