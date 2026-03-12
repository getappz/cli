//! Information Architecture rebuild prompt.

/// System prompt for the IA rebuild AI agent.
pub const SYSTEM: &str = r#"You are a senior UX architect.

You restructure website content into a modern, clean information architecture.

Rules:
- Remove page duplication.
- Merge overlapping content.
- Improve clarity and navigation.
- Return ONLY valid JSON.
- Follow the schema exactly."#;

/// Build the user prompt for IA rebuild.
pub fn build_user_prompt(crawl_summary: &str, classification_json: &str) -> String {
    format!(
        r#"Given the crawl data and classification:

CRAWL DATA:
{crawl_summary}

CLASSIFICATION:
{classification_json}

Tasks:
1. Normalize site structure.
2. Remove unnecessary or duplicate pages.
3. Propose improved route hierarchy.
4. Identify page types: "landing", "standard", "collection", "blog", "contact", or "legal".
5. Identify required new pages if missing (e.g., Contact, About).
6. Propose primary navigation links and footer links.

Return JSON in this exact format:

{{
  "routes": [
    {{
      "path": "/",
      "type": "landing",
      "title": "Home",
      "source_urls": []
    }}
  ],
  "navigation": {{
    "primary": [
      {{ "label": "Home", "href": "/" }}
    ],
    "footer": [
      {{ "label": "Privacy", "href": "/privacy" }}
    ]
  }}
}}"#
    )
}

/// Build the user prompt for create mode.
pub fn build_create_prompt(user_prompt: &str, classification_json: &str) -> String {
    format!(
        r#"A user wants to create a new website with this description:

"{user_prompt}"

CLASSIFICATION:
{classification_json}

Based on the business classification, propose a complete site structure.

Tasks:
1. Propose a clean route hierarchy.
2. Identify page types for each route: "landing", "standard", "collection", "blog", "contact", or "legal".
3. Propose primary navigation links and footer links.
4. Suggest 3-5 pages that would make sense for this type of business.

Return JSON in this exact format:

{{
  "routes": [
    {{
      "path": "/",
      "type": "landing",
      "title": "Home",
      "source_urls": []
    }}
  ],
  "navigation": {{
    "primary": [
      {{ "label": "Home", "href": "/" }}
    ],
    "footer": [
      {{ "label": "Privacy", "href": "/privacy" }}
    ]
  }}
}}"#
    )
}
