#!/usr/bin/env bash
# Scrape a URL via appzcrawl API and save markdown + rawHtml to crawls/<slug>.<ext>
#
# Usage: ./crawl-save.sh [url]
#   url — defaults to https://www.sanshok.com/contact
#
# Requires: curl, jq
# Saves: crawls/<slug>.md, crawls/<slug>.html

set -e

BASE_URL="${1:-https://www.sanshok.com/contact}"
API_URL="${APPZCRAWL_API_URL:-http://127.0.0.1:8797}"
API_KEY="${APPZCRAWL_API_KEY:-appzcrawl_local_dev_key_20260210}"

# Slug from URL: www.sanshok.com/contact -> www.sanshok.com_contact
slug=$(echo "$BASE_URL" | sed -e 's|^https\?://||' -e 's|/*$||' -e 's|/|_|g' -e 's|[^a-zA-Z0-9._-]||g')
[[ -z "$slug" ]] && slug="index"

mkdir -p crawls

body=$(jq -n --arg url "$BASE_URL" '{url: $url, formats: ["markdown", "rawHtml"],maxAge:0}')
resp=$(curl -sS -X POST "${API_URL}/v2/scrape" \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer ${API_KEY}" \
  -d "$body")

success=$(echo "$resp" | jq -r '.success')
if [[ "$success" != "true" ]]; then
  echo "Scrape failed:" >&2
  echo "$resp" | jq . >&2
  exit 1
fi

md=$(echo "$resp" | jq -r '.data.markdown // empty')
html=$(echo "$resp" | jq -r '.data.rawHtml // empty')

if [[ -n "$md" ]]; then
  echo "$md" > "crawls/${slug}.md"
  echo "Wrote crawls/${slug}.md"
fi

if [[ -n "$html" ]]; then
  echo "$html" > "crawls/${slug}.html"
  echo "Wrote crawls/${slug}.html"
fi

if [[ -z "$md" && -z "$html" ]]; then
  echo "No markdown or rawHtml in response" >&2
  echo "$resp" | jq . >&2
  exit 1
fi
