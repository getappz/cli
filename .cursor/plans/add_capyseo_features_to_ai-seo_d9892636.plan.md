---
name: ""
overview: ""
todos: []
---

---

name: Add 32 High-Value SEO Rules to ai-seo (v1.0)

overview: This plan adds 32 prioritized SEO rules from capyseo-core to the ai-seo Rust crate, organized into streaming-safe, full-document, and live-HTTP tiers. Includes Rule Capability Matrix, advisory AI layer, and proper architectural separation.

todos:

  - id: rule-capability-matrix

content: Implement Rule Capability Matrix using bitflags (STREAMING_HTML, FULL_DOCUMENT, LIVE_HTTP, AI_ASSISTED) - CRITICAL: Must be done first

status: pending

  - id: rule-trait-with-capabilities

content: Create Rule trait with RuleMeta including capabilities field. Each rule must declare its capabilities explicitly

status: pending

dependencies:

      - rule-capability-matrix
  - id: rule-architecture

content: Refactor rules.rs into modular rule modules organized by tier (tier_a_streaming, tier_b_document, tier_c_live)

status: pending

dependencies:

      - rule-trait-with-capabilities
  - id: implement-tier-a-rules

content: "Implement Tier A rules (18 streaming-safe): title, meta description, charset, viewport, lang, robots meta, single H1, heading order, empty headings, image alt, image lazy loading, canonical, favicon, OG tags, Twitter card, HTTPS (static), trailing slash, URL length"

status: pending

dependencies:

      - rule-architecture
  - id: implement-tier-b-rules

content: "Implement Tier B rules (8 full-document): keyword density, readability, duplicate content (hash-based), JSON-LD presence, breadcrumb detection, hreflang presence, internal/external link ratio, URL structure/keywords"

status: pending

dependencies:

      - rule-architecture
  - id: implement-tier-c-rules

content: "Implement Tier C rules (6 live HTTP - separate pass): broken links, redirect chains, security headers, mixed content, external script domains, form action security"

status: pending

dependencies:

      - rule-architecture
      - http-client-phase2
  - id: update-models-selectors

content: "Update SeoIssue model: replace line: Option<u32> with selector: Option<String> (CSS-like). Add suggestion: Option<String>. Remove autofix from v1"

status: pending

  - id: update-registry

content: Update issue registry with all 32 rule codes, capabilities, and metadata. Ensure proper weight distribution

status: pending

dependencies:

      - implement-tier-a-rules
      - implement-tier-b-rules
  - id: scoring-simplified

content: "Implement simplified grading profiles: just weight sets per category, no severity mutation, transparent math"

status: pending

  - id: http-client-phase2

content: "Create separate HTTP client module for Phase 2 (live checks): SSRF protection, request budget, SQLite caching, <3s timeout"

status: pending

  - id: ai-advisory-layer

content: "Implement advisory AI layer (post-analysis, optional, cached): AiProvider trait with suggest() method taking AiRequest enum. Never affects rule execution or scoring"

status: pending

  - id: reporter-streaming-contract

content: "Refactor reporters to streaming contract: begin_site(), report_page(), finish(). Enable streaming output, prevent large buffers"

status: pending

  - id: golden-fixtures

content: "Create golden fixture tests: fixtures/page.html + expected.json for each rule. Ensure idempotency and determinism"

status: pending

dependencies:

      - update-registry
  - id: unit-tests

content: Add comprehensive unit tests for all 32 rule modules with golden fixtures

status: pending

dependencies:

      - golden-fixtures
  - id: integration-tests

content: Add integration tests for full analysis pipeline, verify two-phase execution (HTML analysis, then optional live checks)

status: pending

dependencies:

      - unit-tests
  - id: add-dependencies

content: "Add required dependencies: url crate, bitflags for capabilities. No external readability crate (implement custom)"

status: pending

  - id: update-documentation

content: "Update README.md with all 32 rules organized by tier, document Rule Capability Matrix, explain two-phase analysis, document AI advisory layer"

status: pending

dependencies:

      - integration-tests

---

# Add 32 High-Value SEO Rules to ai-seo (v1.0)

## Executive Summary

This plan adds 32 prioritized SEO rules from capyseo-core, organized into three tiers based on execution requirements. Critical architectural improvements include a Rule Capability Matrix, advisory-only AI integration, and proper separation of streaming, document-level, and live-HTTP analysis.

## Critical Architectural Fixes

### 1. Rule Capability Matrix (MANDATORY - Do First)

**Problem**: Not all SEO rules are equal. Some require streaming HTML, full document context, live HTTP, or AI assistance. Without explicit capability modeling, rules will fail in wrong contexts.

**Solution**: Introduce capability flags before implementing any new rules.

```rust
use bitflags::bitflags;

bitflags! {
    pub struct RuleCapabilities: u32 {
        const STREAMING_HTML = 0b0001;  // Can execute during streaming parse
        const FULL_DOCUMENT  = 0b0010;  // Needs complete document in memory
        const LIVE_HTTP      = 0b0100;  // Requires HTTP requests
        const AI_ASSISTED    = 0b1000;  // Can use AI for suggestions (advisory only)
    }
}

pub struct RuleMeta {
    pub code: &'static str,
    pub category: RuleCategory,
    pub severity: Severity,
    pub capabilities: RuleCapabilities,
    pub weight: u8,
}
```

**Why this matters**:

- Prevents impossible rules from running in streaming mode
- Enables fast/slow passes (Phase 1: streaming, Phase 2: document, Phase 3: HTTP)
- Allows feature gating (`--no-http`, `--no-ai`)
- Makes CI deterministic
- Prevents performance regressions

### 2. Two-Phase Analysis Architecture

**Phase 1: HTML Analysis** (fast, pure, offline)

- Execute rules with `STREAMING_HTML` capability during parse
- Execute rules with `FULL_DOCUMENT` capability after parse completes
- No HTTP requests, no AI calls
- Deterministic, cacheable results

**Phase 2: Live Checks** (async, cached, optional)

- Execute rules with `LIVE_HTTP` capability
- SSRF protection (block RFC1918, localhost)
- Request budget per site
- Result caching in SQLite
- Timeout < 3s per request

### 3. AI as Advisory Layer (Never Required)

**Problem**: Embedding AI into rule execution breaks determinism, makes CI flaky, increases latency.

**Solution**: AI is post-analysis, advisory-only, optional, cached.

```
HTML → Rules → Issues
                ↓
           (optional)
            AI layer
                ↓
        Suggestions only
```

**AI Contract**:

```rust
pub enum AiRequest {
    MetaDescription { text: String },
    Title { text: String },
    ImageAlt { context: String },
    ContentSummary { text: String },
}

pub trait AiProvider {
    fn suggest(&self, input: AiRequest) -> Result<AiSuggestion>;
}
```

**AI must never**:

- Decide if an issue exists
- Affect scoring
- Be required to run a rule
- Block rule execution

### 4. Streaming Reporter Contract

**Problem**: `Reporter::generate(&[SeoReport]) -> String` requires buffering all reports in memory.

**Solution**: Streaming contract enables progress bars and large-site analysis.

```rust
pub trait Reporter {
    fn begin_site(&mut self, meta: &SiteMeta);
    fn report_page(&mut self, report: &SeoReport);
    fn finish(&mut self) -> Result<()>;
}
```

## Rule Organization: 32 Rules in 3 Tiers

### Tier A: Streaming-Safe (18 rules) - Implement First

High-value rules that can execute during streaming HTML parse:

1. **meta-title** ✓ (existing - enhance with length checks)
2. **meta-description** ✓ (existing - enhance with length checks)
3. **meta-charset** - UTF-8 charset declaration
4. **meta-viewport** - Mobile viewport configuration
5. **lang-attribute** - HTML lang attribute
6. **robots-meta** - Robots meta tags (noindex, nofollow)
7. **single-h1** ✓ (existing - enhance)
8. **heading-order** - Validate H1→H2→H3 hierarchy
9. **empty-heading** - Detect empty headings
10. **image-alt** ✓ (existing)
11. **image-lazy-load** ✓ (existing)
12. **canonical-url** ✓ (existing)
13. **favicon** - Favicon link tags
14. **og-tags** - Open Graph meta tags (og:title, og:description, og:image, og:url, og:type)
15. **twitter-card** - Twitter Card meta tags
16. **https-static** - HTTPS usage (static URL check, no HTTP request)
17. **trailing-slash** - Trailing slash consistency
18. **url-length** - URL path length validation

**Capabilities**: `STREAMING_HTML`

### Tier B: Full Document Context (8 rules)

Rules that need complete document in memory but no HTTP:

19. **keyword-density** - Analyze keyword usage and density
20. **readability** - Flesch-Kincaid readability scoring
21. **duplicate-content** - Hash-based duplicate detection (H1s, title/H1 matches)
22. **json-ld-presence** - JSON-LD structured data presence (not full validation)
23. **breadcrumb-detection** - Breadcrumb structured data detection
24. **hreflang-presence** - Hreflang tag presence validation
25. **link-ratio** - Internal vs external link ratio
26. **url-structure-keywords** - URL structure and keyword presence

**Capabilities**: `FULL_DOCUMENT`

### Tier C: Live HTTP (6 rules) - Separate Pass

Rules requiring HTTP requests (Phase 2 only):

27. **broken-links** - Check for broken internal/external links
28. **redirect-chains** - Detect redirect chains
29. **security-headers** - Check security headers (CSP, HSTS, etc.)
30. **mixed-content** - Detect mixed HTTP/HTTPS content
31. **external-scripts** - Analyze external script domains
32. **form-security** - Check form action security

**Capabilities**: `LIVE_HTTP`

**Protection Requirements**:

- SSRF protection (block RFC1918, localhost)
- Request budget per site (max N requests)
- SQLite result caching
- Timeout < 3s per request
- Feature flag: `--no-http` to skip

## Implementation Plan

### Phase 1: Rule Capability Matrix & Architecture (Week 1)

**1.1 Rule Capability Matrix**

- **File**: `crates/ai-seo/src/rules/capabilities.rs`
- Implement `RuleCapabilities` bitflags
- Create `RuleMeta` struct with capabilities field
- Document capability semantics

**1.2 Rule Trait Refactoring**

- **File**: `crates/ai-seo/src/rules/trait.rs`
- Create `Rule` trait with `RuleMeta` including capabilities
- Update rule execution to check capabilities before running
- Add capability-based rule filtering

**1.3 Rule Module Structure**

- **File**: `crates/ai-seo/src/rules/`
- Organize by tier:
  - `tier_a_streaming.rs` - 18 streaming rules
  - `tier_b_document.rs` - 8 document rules
  - `tier_c_live.rs` - 6 live HTTP rules
- Each rule module exports rules with explicit capabilities

### Phase 2: Tier A Rules - Streaming-Safe (Week 2)

**2.1 Enhance Existing Rules**

- Update existing 8 rules with proper capabilities
- Add length/validation checks where missing
- Ensure all work correctly during streaming parse

**2.2 Implement New Tier A Rules**

- meta-charset, meta-viewport, lang-attribute, robots-meta
- heading-order, empty-heading
- favicon
- og-tags, twitter-card
- https-static, trailing-slash, url-length

**2.3 Golden Fixtures**

- Create `fixtures/` directory
- Add `page.html` + `expected.json` for each rule
- Ensure idempotency and determinism

### Phase 3: Tier B Rules - Full Document (Week 3)

**3.1 Implement Tier B Rules**

- keyword-density (custom implementation, no external crate)
- readability (Flesch-Kincaid, custom implementation)
- duplicate-content (hash-based)
- json-ld-presence, breadcrumb-detection
- hreflang-presence, link-ratio, url-structure-keywords

**3.2 Document Context Collection**

- After streaming parse, collect full document state
- Enable Tier B rules to access complete document
- Maintain streaming performance for Tier A

### Phase 4: Models, Scoring, Registry (Week 4)

**4.1 Update Data Models**

- **File**: `crates/ai-seo/src/models.rs`
- Replace `line: Option<u32>` with `selector: Option<String>` (CSS-like)
- Add `suggestion: Option<String>`
- Remove `autofix` from v1 (too complex)

**4.2 Simplified Grading Profiles**

- **File**: `crates/ai-seo/src/scoring.rs`
- Profiles as weight sets only:
  ```rust
  pub struct GradingProfile {
      pub weights: HashMap<RuleCategory, u32>,
      pub max_score: u32,
  }
  ```

- No severity mutation
- Transparent math

**4.3 Registry Updates**

- **File**: `crates/ai-seo/src/registry.rs`
- Register all 32 rules with capabilities
- Proper weight distribution
- Category mapping

### Phase 5: Reporters - Streaming Contract (Week 5)

**5.1 Refactor Reporter Trait**

- **File**: `crates/ai-seo/src/reporters/mod.rs`
- New contract: `begin_site()`, `report_page()`, `finish()`
- Enable streaming output
- Prevent large in-memory buffers

**5.2 Implement Reporters**

- JSON reporter (streaming)
- Console reporter (streaming with progress)
- CSV reporter (streaming)
- SARIF reporter (batch, but efficient)

### Phase 6: AI Advisory Layer (Week 6)

**6.1 AI Provider Trait**

- **File**: `crates/ai-seo/src/ai/mod.rs`
- `AiProvider` trait with `suggest(AiRequest) -> Result<AiSuggestion>`
- `AiRequest` enum (not individual methods)
- Never called during rule execution

**6.2 AI Integration (Optional)**

- Post-analysis AI suggestions
- Aggressive caching in SQLite
- Feature flag: `--no-ai` to skip
- Never affects scoring or issue detection

### Phase 7: Tier C Rules - Live HTTP (Week 7)

**7.1 HTTP Client Module**

- **File**: `crates/ai-seo/src/http/`
- SSRF protection (block RFC1918, localhost)
- Request budget per site
- SQLite result caching
- Timeout < 3s per request

**7.2 Implement Tier C Rules**

- broken-links, redirect-chains
- security-headers, mixed-content
- external-scripts, form-security

**7.3 Two-Phase Execution**

- Phase 1: HTML analysis (Tier A + B)
- Phase 2: Live checks (Tier C, optional, feature-flagged)

### Phase 8: Testing & Stabilization (Week 8)

**8.1 Golden Fixture Tests**

- Every rule has fixture test
- Idempotency verification
- Determinism checks

**8.2 Integration Tests**

- Full pipeline with all tiers
- Two-phase execution verification
- Performance benchmarks

**8.3 Documentation**

- README with all 32 rules by tier
- Rule Capability Matrix explanation
- Two-phase analysis documentation
- AI advisory layer usage

## File Structure

```
crates/ai-seo/src/
├── lib.rs
├── analyzer.rs (two-phase execution)
├── models.rs (selector instead of line)
├── scoring.rs (simplified profiles)
├── registry.rs (with capabilities)
├── rules/
│   ├── mod.rs
│   ├── capabilities.rs (NEW - bitflags)
│   ├── trait.rs (NEW - Rule trait with capabilities)
│   ├── tier_a_streaming.rs (18 rules)
│   ├── tier_b_document.rs (8 rules)
│   └── tier_c_live.rs (6 rules)
├── reporters/
│   ├── mod.rs (streaming contract)
│   ├── json.rs
│   ├── console.rs
│   ├── csv.rs
│   └── sarif.rs
├── ai/
│   ├── mod.rs (advisory only)
│   └── provider.rs (trait)
├── http/
│   ├── mod.rs (Phase 2 only)
│   ├── client.rs (SSRF protection)
│   └── cache.rs
├── fixtures/ (NEW)
│   ├── tier_a/
│   ├── tier_b/
│   └── tier_c/
├── fix_plan.rs (existing)
├── routing.rs (existing)
├── mutation.rs (existing)
├── diff.rs (existing)
├── preview.rs (existing)
├── aggregation.rs (existing)
└── db.rs (existing)
```

## Dependencies to Add

```toml
[dependencies]
# Existing...
bitflags = "2.0"  # For Rule Capability Matrix
url = "2.5"       # For URL parsing
# Note: Readability and keyword density implemented custom (no external crates)
```

## Success Criteria

- [ ] Rule Capability Matrix implemented and enforced
- [ ] All 32 rules implemented with correct capabilities
- [ ] Two-phase analysis working (HTML → optional HTTP)
- [ ] AI advisory layer working (post-analysis, optional, cached)
- [ ] Streaming reporters working (no large buffers)
- [ ] Golden fixture tests passing for all rules
- [ ] Simplified grading profiles (weights only)
- [ ] Selectors instead of line numbers
- [ ] SSRF protection for HTTP checks
- [ ] Backward compatible with existing code
- [ ] Comprehensive test coverage (>80%)
- [ ] Documentation complete

## Estimated Timeline

| Phase | Duration | Description |

|-------|----------|-------------|

| 1 | 1 week | Rule Capability Matrix & Architecture |

| 2 | 1 week | Tier A Rules (18 streaming-safe) |

| 3 | 1 week | Tier B Rules (8 full-document) |

| 4 | 1 week | Models, Scoring, Registry |

| 5 | 1 week | Streaming Reporters |

| 6 | 1 week | AI Advisory Layer |

| 7 | 1 week | Tier C Rules (6 live HTTP) |

| 8 | 1 week | Testing & Stabilization |

**Total: 8 weeks** (more realistic than original 4-5 weeks)

## Key Architectural Decisions

1. **Rule Capability Matrix First**: Prevents architectural mistakes
2. **Two-Phase Analysis**: Separates fast (HTML) from slow (HTTP)
3. **AI Advisory Only**: Maintains determinism and CI stability
4. **Streaming Reporters**: Enables large-site analysis
5. **Simplified Profiles**: Transparent, explainable scoring
6. **Selectors Not Lines**: Accurate with streaming parsers
7. **Golden Fixtures**: Prevents silent regressions
8. **32 Rules Max**: High signal-to-noise ratio

## What's Deferred to v1.1+

- Custom rules API (runtime)
- WASM-based custom rules
- Full JSON-LD validation (presence only in v1)
- Mobile-specific rules requiring CSS parsing
- Content freshness (needs timestamps)
- Advanced autofix (too complex for v1)

This plan creates a **credible, production-grade Rust SEO engine**, not a TypeScript port.