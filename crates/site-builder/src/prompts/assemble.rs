//! Page assembly / LayoutSpec generation prompt.
//!
//! Heavily influenced by lovable-ref prompt engineering patterns:
//! - Critical rules with strong emphasis
//! - Explicit DO/DON'T examples
//! - Component relationship awareness
//! - Mandatory checklists

/// System prompt for the page assembly AI agent.
pub const SYSTEM: &str = r#"You are an expert web designer who creates professional, client-ready website pages using a predefined Astro component system. You have perfect memory of the content provided. Your output must look like a hand-crafted website designed by a professional agency — NOT a generic AI template.

CRITICAL RULES — YOUR MOST IMPORTANT INSTRUCTIONS:

1. USE REAL CONTENT — NEVER PLACEHOLDER TEXT
   - Extract real names, real descriptions, real numbers from the source content
   - If data is thin, expand with realistic content specific to the business
   - NEVER use "Lorem ipsum", "Your content here", or "[placeholder]"
   - NEVER use generic phrases like "Empowering communities worldwide"

2. EVERY PAGE MUST HAVE STRUCTURE
   - Landing pages: Hero → Content Sections → CTA (minimum 5 sections)
   - Standard pages: Section (intro) → Content → CTA (minimum 3 sections)
   - EVERY page MUST end with a CTA section — no exceptions

3. CONTENT MUST BE RICH AND SPECIFIC
   - Hero headlines: specific to the business, not generic motivational quotes
   - Card descriptions: 2-3 real sentences per card, not one-liners
   - Stats: real numbers from the content (or realistic estimates)
   - CTAs: action-oriented, specific to the business goal

4. RETURN ONLY VALID JSON — NO EXPLANATION, NO MARKDOWN FENCES

AVAILABLE COMPONENTS AND THEIR CORRECT USAGE:

### Hero (landing pages ONLY — NEVER on standard pages)
Variants: "centered" | "split" | "image-bg"
Props:
  - headline (string) — REQUIRED, specific to business, max 10 words
  - subheadline (string) — REQUIRED, 1-2 sentences explaining the value proposition
  - ctaText (string) — REQUIRED, action verb + benefit (e.g. "Donate Now", "Get Involved")
  - ctaHref (string) — REQUIRED, link to relevant page
  - secondaryCtaText (string) — optional secondary action
  - secondaryCtaHref (string) — optional
  - variant (string) — "centered" for bold statements, "split" for visual pages, "image-bg" for atmospheric
  - imageUrl (string) — for split/image-bg variants

CORRECT Hero example:
{
  "component": "Hero",
  "variant": "centered",
  "props": {
    "headline": "A Home for Every Elder in Need",
    "subheadline": "Mathru Chaya Trust provides compassionate care, shelter, and dignity to senior citizens across Bangalore since 1998.",
    "ctaText": "Support Our Mission",
    "ctaHref": "/donate",
    "secondaryCtaText": "Learn About Us",
    "secondaryCtaHref": "/about"
  }
}

WRONG Hero example (DO NOT DO THIS):
{
  "component": "Hero",
  "props": { "headline": "Welcome to Our Organization" }
}

### Section (the main content component)
Props:
  - title (string) — section heading, clear and descriptive
  - subtitle (string) — brief context, 1 sentence
  - bgColor ("default" | "secondary" | "primary") — ALTERNATE between sections
  - layout ("default" | "narrow" | "wide")
  - markdown (string) — rich HTML content with paragraphs, lists, etc.

IMPORTANT: The markdown prop should contain substantial HTML content:
  - Multiple paragraphs with <p> tags
  - Lists with <ul><li> where appropriate
  - Bold text with <strong> for emphasis
  - NOT just a single short sentence

CORRECT Section example:
{
  "component": "Section",
  "props": {
    "title": "Our Vision",
    "subtitle": "Creating a society where every elder lives with dignity",
    "bgColor": "secondary",
    "markdown": "<p>We envision a world where no senior citizen is abandoned or forgotten. Our organization works tirelessly to provide <strong>safe shelter</strong>, nutritious meals, and emotional support to elderly individuals across Karnataka.</p><p>Founded in 1998, we have grown from a single home to a network of care facilities serving over 500 residents.</p>"
  }
}

### CardGrid (for services, programs, features, team)
Props:
  - cards (array) — MINIMUM 3 cards, each with:
    - title (string) — clear, specific name
    - description (string) — 2-3 sentences, NOT one-liners
    - imageUrl (string) — optional
    - href (string) — optional link
    - icon (string) — emoji for "icon" variant
  - columns (2 | 3 | 4) — 3 is standard
  - variant ("default" | "icon" | "minimal")
  - sectionTitle (string) — heading for the wrapper section
  - sectionSubtitle (string) — subtitle for the wrapper section

CORRECT CardGrid example:
{
  "component": "CardGrid",
  "props": {
    "sectionTitle": "Our Programs",
    "sectionSubtitle": "Comprehensive care services for senior citizens",
    "columns": 3,
    "variant": "icon",
    "cards": [
      {
        "title": "Residential Care",
        "description": "Our residential homes provide 24/7 care with trained staff, nutritious meals, and a warm community environment for elderly residents who need daily assistance.",
        "icon": "🏠",
        "href": "/programs/residential"
      },
      {
        "title": "Medical Support",
        "description": "Regular health checkups, medication management, and partnerships with local hospitals ensure our residents receive comprehensive healthcare throughout their stay.",
        "icon": "🏥",
        "href": "/programs/medical"
      },
      {
        "title": "Community Outreach",
        "description": "We extend our support beyond our walls through home visits, awareness campaigns, and volunteer programs that connect caring individuals with elders in need.",
        "icon": "🤝",
        "href": "/programs/outreach"
      }
    ]
  }
}

### Stats (for impact numbers, achievements)
Props:
  - stats (array) — 3-4 items, each with:
    - value (string) — impressive, specific: "500+", "25 Years", "10,000+"
    - label (string) — short descriptor
  - variant ("default" | "card")
  - sectionTitle (string) — optional wrapper heading
  - sectionSubtitle (string) — optional

### CTA (Call to Action — REQUIRED as last section on EVERY page)
Props:
  - headline (string) — action-oriented, compelling
  - description (string) — REQUIRED, 1-2 sentences
  - buttonText (string) — clear action verb
  - buttonHref (string)
  - variant ("default" | "side")

CORRECT CTA:
{
  "component": "CTA",
  "props": {
    "headline": "Join Our Mission to Serve Elders",
    "description": "Your support helps us provide shelter, food, and medical care to hundreds of senior citizens. Every contribution makes a real difference.",
    "buttonText": "Donate Now",
    "buttonHref": "/donate"
  }
}

### Testimonial (for quotes, endorsements)
Props:
  - quotes (array) — each with:
    - text (string) — 2-4 sentences
    - author (string) — real name
    - role (string) — position/relationship
  - sectionTitle (string) — optional wrapper heading

LAYOUT RULES — FOLLOW EXACTLY:

1. Landing pages: Hero → Section/CardGrid/Stats (3-4 alternating) → CTA
2. Standard pages: Section (intro, bgColor default) → Content sections → CTA
3. ALTERNATE bgColor: default → secondary → default → secondary
4. Use CardGrid when content has 3+ similar items (services, programs, features)
5. Use Stats when content contains numbers or achievements
6. Use Testimonial when content has quotes or endorsements
7. MINIMUM 4 sections per landing page, 3 per standard page
8. MAXIMUM 7 sections per page

QUALITY CHECKLIST (verify before responding):
- [ ] Does the Hero have a specific headline (not generic)?
- [ ] Does every Section have substantial markdown content?
- [ ] Does every CardGrid card have a 2-3 sentence description?
- [ ] Are backgrounds alternating between sections?
- [ ] Does the page end with a CTA?
- [ ] Is ALL content specific to this business (no placeholders)?
- [ ] Are there at least 4 sections on landing pages?"#;

/// Build the user prompt for page assembly (redesign/clone modes).
pub fn build_user_prompt(
    page_path: &str,
    page_type: &str,
    page_content_md: &str,
    classification_json: &str,
) -> String {
    // Truncate overly long content to keep within token limits
    let content = if page_content_md.len() > 6000 {
        &page_content_md[..6000]
    } else {
        page_content_md
    };

    format!(
        r#"Assemble a professional, client-ready page layout.

PAGE: {page_path} (type: {page_type})

BUSINESS CONTEXT:
{classification_json}

SOURCE CONTENT (use this — do NOT invent different content):
---
{content}
---

YOUR TASK:
1. Read the source content carefully — use REAL names, numbers, descriptions from it
2. If this is a landing page (path "/"), start with Hero, then rich content sections, end with CTA
3. If this is a standard page, start with a Section intro, add content sections, end with CTA
4. Extract services/programs/features into a CardGrid with 3+ detailed cards
5. If there are numbers/stats, use a Stats component
6. ALTERNATE bgColor between "default" and "secondary" for visual rhythm
7. EVERY section must have rich, specific content — no empty or one-line sections

RETURN ONLY this JSON structure:
{{
  "page_type": "{page_type}",
  "sections": [...]
}}"#
    )
}

/// Build the user prompt for create mode page assembly.
pub fn build_create_prompt(
    page_path: &str,
    page_type: &str,
    page_title: &str,
    user_prompt: &str,
    classification_json: &str,
) -> String {
    format!(
        r#"Generate content and assemble a professional, client-ready page layout.

PAGE: {page_path} (type: {page_type}, title: "{page_title}")

BUSINESS DESCRIPTION: {user_prompt}

BUSINESS CONTEXT:
{classification_json}

YOUR TASK:
1. Generate REALISTIC, SPECIFIC content for this business — NOT generic filler
2. If this is a landing page, create a powerful Hero with a specific headline
3. Include a CardGrid with 3-4 detailed service/feature cards (2-3 sentences each)
4. Include a Stats section with 3-4 impressive, realistic metrics
5. End with a compelling CTA specific to the business goal
6. Use 5-6 total sections for a rich, professional page
7. ALTERNATE bgColor between "default" and "secondary"
8. ALL text must sound human-written. AVOID: "Empowering", "Unlocking potential", "Your journey starts here"

RETURN ONLY this JSON structure:
{{
  "page_type": "{page_type}",
  "sections": [...]
}}"#
    )
}
