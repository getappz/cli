//! Business classification prompt.

/// System prompt for the business classification AI agent.
pub const SYSTEM: &str = r#"You are a senior digital strategy consultant specializing in website redesign.

You analyze crawled website data and produce structured strategic outputs.

Rules:
- Return ONLY valid JSON.
- No commentary, no markdown, no explanations.
- Follow the schema exactly."#;

/// Build the user prompt for business classification.
pub fn build_user_prompt(crawl_summary: &str, branding_json: Option<&str>) -> String {
    let branding_section = branding_json
        .map(|b| format!("\n\nExisting branding data:\n{}", b))
        .unwrap_or_default();

    format!(
        r#"Analyze the following website crawl data:

{crawl_summary}{branding_section}

Tasks:
1. Identify primary business category.
2. Identify secondary category (if any).
3. Identify target audiences (max 4).
4. Identify brand tone (formal, charitable, corporate, startup, playful, etc).
5. Identify content density (low, medium, high).
6. Suggest the most appropriate pre-built theme: "nonprofit", "corporate", "startup", or "minimal".
7. Suggest appropriate layout type (storytelling, content-rich, minimal, portfolio).
8. Identify trust indicators required (certifications, testimonials, impact metrics, etc).
9. Identify conversion goals (donate, sign up, contact, purchase, etc).

Return JSON in this exact format:

{{
  "primary_category": "",
  "secondary_category": "",
  "audiences": [],
  "brand_tone": "",
  "content_density": "",
  "suggested_theme": "",
  "layout_type": "",
  "trust_elements": [],
  "conversion_goals": []
}}"#
    )
}

/// Build the user prompt for create mode (from a text description).
pub fn build_create_prompt(user_prompt: &str) -> String {
    format!(
        r#"A user wants to create a new website with the following description:

"{user_prompt}"

Based on this description, perform the same analysis as if you had crawled an existing site.

Tasks:
1. Identify primary business category.
2. Identify secondary category (if any).
3. Identify target audiences (max 4).
4. Identify brand tone (formal, charitable, corporate, startup, playful, etc).
5. Identify content density (low, medium, high).
6. Suggest the most appropriate pre-built theme: "nonprofit", "corporate", "startup", or "minimal".
7. Suggest appropriate layout type (storytelling, content-rich, minimal, portfolio).
8. Identify trust indicators required (certifications, testimonials, impact metrics, etc).
9. Identify conversion goals (donate, sign up, contact, purchase, etc).

Return JSON in this exact format:

{{
  "primary_category": "",
  "secondary_category": "",
  "audiences": [],
  "brand_tone": "",
  "content_density": "",
  "suggested_theme": "",
  "layout_type": "",
  "trust_elements": [],
  "conversion_goals": []
}}"#
    )
}
