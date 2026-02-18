# Appzcrawl API

Hono-based API for the **v2** Firecrawl-compatible surface, running on Cloudflare Workers. Uses D1, R2, and Drizzle. No v0, v1, or admin routes.

## Run locally

```bash
pnpm dev
```

Uses `wrangler.devel.jsonc`. Default port: **8797**.

For PDF `formats: ["json"]` (LLM extraction), create `.dev.vars` with `SARVAM_API_KEY` (see `.dev.vars.example`). **Workers AI requires remote mode:** use `pnpm dev:remote` instead of `pnpm dev` for json extraction (AI binding does not work in local-only wrangler dev).

## Deploy

- **Devel:** `pnpm deploy` (uses `wrangler.devel.jsonc`)
- **Prod:** `pnpm deploy:prod` (uses `wrangler.prod.jsonc`)

## Database (D1)

- Create DB: `wrangler d1 create appzcrawl-db` (then set `database_id` in wrangler config).
- Local migrations: `pnpm db:migrate:local`
- Remote (devel): `pnpm db:migrate:devel`
- Remote (prod): `pnpm db:migrate:prod`

## R2

Binding `BUCKET` is configured in wrangler. Create bucket `appzcrawl-bucket` (or match name in config) and use it in workers for storing scraped/crawl output.

## v2-only, copy-and-adapt

This service implements **only the v2 API** from Firecrawl. Routes, middleware, and controllers were copied from [firecrawl/apps/api](https://github.com/mendableai/firecrawl) and adapted to:

- **Hono** instead of Express
- **D1 + Drizzle** instead of Postgres/Supabase
- **R2** instead of GCS
- **Cloudflare Queues** (stubbed) instead of Redis/BullMQ
- **API key auth** via D1 `api_keys` or env `API_KEY` for dev

No v0, v1, or admin routes are included.
