---
name: website-builder
description: Generate complete, production-ready React (.jsx) websites and web apps from a description — redesign existing sites — or use a reference/inspiration site as a design template. Use this skill whenever a user asks to build, create, generate, or design a website, landing page, portfolio, business site, marketing page, or interactive web app. Also use when a user pastes existing code or a URL and asks to redesign, modernize, refresh, or improve it. Also use when a user shares a site they like and says "make mine look like this", "use this as a template", "inspired by", "similar style to", or pastes a URL as a visual reference. Covers landing pages, portfolio/personal sites, business/company sites, and full interactive web apps. Always use this skill rather than writing ad-hoc React code.
---

# Website Builder Skill

Generate complete, visually stunning, production-ready React (.jsx) websites from a user's description — or redesign an existing site.

## Output Directory

**Always** create each website under `~/workspace/sites/dev/`:
- Create a subfolder per site: `~/workspace/sites/dev/<site-name>/` (or `~/workspace/sites/dev/<site-name>-redesign/` for redesigns)
- Include a runnable setup: `index.html` (loads React + Babel from CDN) + `App.jsx` + optional `README.md`
- Run with `npx serve .` from the site folder

## Mode Selection

Determine which mode applies before proceeding:

| Signal | Mode |
|--------|------|
| User describes a site to build from scratch | → **[Build Mode](#build-mode)** |
| User pastes HTML/JSX/CSS code to improve | → **[Redesign Mode](#redesign-mode)** |
| User shares a URL and asks to redesign/improve it | → **[Redesign Mode](#redesign-mode)** |
| User says "modernize", "refresh", "make this look better", "restyle" | → **[Redesign Mode](#redesign-mode)** |
| User shares a URL/site they like as inspiration | → **[Template Mode](#template-mode)** |
| User says "make mine look like X", "inspired by", "same style as" | → **[Template Mode](#template-mode)** |
| User provides both a reference site AND their own content | → **[Template Mode](#template-mode)** |

---

## Redesign Mode

### Step R1: Audit the Existing Site

Accept input as **one of**:
- Pasted HTML/JSX/CSS code
- A URL (fetch with `web_fetch` to get the page source and screenshot if possible)
- A description of what the current site looks like

Perform a structured audit — identify what to **keep** vs. **transform**:

**Content inventory** (always preserve):
- All text copy, headings, CTAs
- Navigation structure and page sections
- Core functionality and links
- Brand name and any explicit brand colors/fonts the user wants to keep
- **Original logo** — When redesigning from a URL, fetch the page HTML (e.g. `curl -s URL | grep -iE '<img|logo|src='`) and extract the logo `src` path (e.g. `images/logo.jpg`); use the original logo URL (base URL + path) in the redesign
- **Existing content images** — Extract and use all images that are required for the content: company/division logos, section imagery, background images. Map each to its section (e.g. company logos → business cards; `bg.jpg` / `bg.png` → hero or section backgrounds)

**Design problems to fix** (replace entirely):
- Outdated visual patterns (table layouts, inline styles, Bootstrap defaults, boxy cards)
- Generic or clashing typography
- Poor color hierarchy or low contrast
- Missing animations and visual depth
- Non-responsive or cramped layouts
- Overuse of borders, shadows, or gradients without intention

### Step R2: Brief the User (optional, only if ambiguous)

If it's unclear what direction to take, ask **one** focused question, e.g.:
- "Should I keep your existing color scheme or reimagine it entirely?"
- "Any sections you want added or removed in the redesign?"

Otherwise, proceed confidently and document your choices in the output summary.

### Step R3: Choose a Redesign Direction

Pick a design direction that **elevates** the original content — do not just clean it up. Ask: *what could this site become if it were designed with real intentionality?*

- Identify the site's purpose and audience, then match the aesthetic to them
- Choose a direction that contrasts with the original's weaknesses (e.g., if it was cluttered → go generous and spacious; if it was dull → go bold and typographic)
- Preserve any brand constraints (colors, tone) the user cares about — transform everything else
- Apply the same aesthetic rules as Build Mode: distinctive fonts, committed palette, motion choreography

### Step R4: Rebuild as React

Reconstruct the site as a complete `.jsx` file:
- **Preserve all content** — same copy, same sections, same navigation
- **Use original images** — When the source is a URL, parse the HTML for all `<img>` tags and `background-image` URLs. Use the logo in the nav and footer; use company/division images in their corresponding cards or sections; use background images (`bg.jpg`, `bg.png`, etc.) for hero or section visuals. Do not replace content images with placeholders or gradients
- **Blend images with containers** — Ensure images do not appear as harsh cutouts: (1) Section/background images: add a gradient overlay (e.g. `linear-gradient(to right, var(--surface), transparent)`) at the edge where they meet adjacent content; use `overflow: hidden`, `border-radius`, and soft `box-shadow`; (2) Logos on hero/nav: wrap in a theme-aware background (`--logo-bg`) when nav is transparent so they sit on the hero without floating; (3) Company logos in cards: avoid bordered boxes; use `border-radius` and a subtle shadow so they feel integrated with the card surface
- **Replace all styling** — new layout system, new color palette, new typography, new spacing
- **Elevate interactions** — add scroll reveals, hover states, and transitions that weren't there before
- **Add theme switcher** — include light/dark/system with system preference as default (see [Theme Switcher](#theme-switcher))
- **Fix responsive issues** — ensure it works flawlessly on mobile
- Follow all the same [Code Quality Rules](#code-quality-rules) as Build Mode

### Step R5: Output

1. Write the complete `.jsx` file — no truncation
2. Create `~/workspace/sites/dev/<site-name>-redesign/` with `index.html`, `App.jsx`, and optional `README.md`
3. Present with `present_files`
4. Write a short **"What changed"** summary covering: design direction chosen, typography/color decisions, layout improvements, and interactions added

---

## Template Mode

Use a reference site as a design template — extract its visual DNA, then apply it to the user's own content.

**Key distinction from Redesign Mode**: In Redesign Mode, the user's *existing site* is the input and gets transformed. In Template Mode, a *third-party site* is the style reference, and the user's *content/brief* is what gets built. The reference site's content is never copied — only its design language.

### Step T1: Ingest the Reference Site

Accept the reference as one of:
- A URL → use `web_fetch` to retrieve the page HTML/CSS
- A screenshot or description of the site's visual style
- A named site (e.g., "like Linear" / "like Stripe") → use your knowledge of that site's aesthetic

### Step T2: Extract the Design DNA

Analyze the reference and document the following — this becomes the style spec for the build:

**Layout patterns**
- Overall page structure (full-bleed sections, constrained content width, sidebar, etc.)
- Hero composition (split, centered, diagonal, offset image, etc.)
- Grid and spacing rhythm (tight/dense vs. airy/generous)
- Any signature layout devices (overlapping elements, sticky sidebars, horizontal scroll, etc.)

**Visual language**
- Color role mapping: primary background, surface, text, accent, CTA button color
- Approximate font weight and size contrast between headings and body
- Border/divider usage (or absence)
- Card style (outlined, filled, elevated, ghost, none)
- Image treatment (full-bleed, masked, illustrated, none)

**Interaction style**
- Animation intensity (subtle fades vs. dramatic reveals vs. none)
- Hover effects on links, buttons, cards
- Scroll behavior (parallax, sticky elements, progress indicators)
- Navigation style (transparent overlay, solid bar, sidebar, hamburger-only)

**Tone**
- Overall feel: minimal, editorial, corporate, playful, luxurious, technical, etc.
- Typography mood: serif/display-heavy, mono/technical, clean sans, expressive script

> Do **not** copy fonts, brand colors, or specific imagery from the reference site. Derive the *pattern* and reimplement with original choices. For example: "uses high-contrast serif display type" → choose a different serif that fits the user's brand.

### Step T3: Gather the User's Content

If not already provided, ask for:
- **What the site is for** (brand name, product/service, purpose)
- **Key sections needed** (if different from the reference site's structure)
- **Any brand constraints** (colors, fonts, tone they want to keep)
- **Content** (copy, headlines, features — or Claude will write purposeful placeholder copy)

If the user's brief is already rich, skip asking and proceed.

### Step T4: Build with the Extracted Style Spec

Build the site exactly as in Build Mode, but using the extracted design DNA as the creative brief instead of inventing a direction from scratch:

- Translate each extracted pattern into concrete CSS/JSX decisions
- Adapt the layout structure to the user's content (don't force the reference's section order if it doesn't fit)
- Reimplement the interaction style with original code
- Write real copy for the user's context — never reuse the reference site's text
- Follow all [Code Quality Rules](#code-quality-rules)

### Step T5: Output

1. Write the complete `.jsx` file — no truncation
2. Create `~/workspace/sites/dev/<site-name>/` with `index.html`, `App.jsx`, and optional `README.md`
3. Present with `present_files`
4. Write a short **"Design DNA extracted"** summary listing: layout patterns borrowed, color/type decisions made, interactions replicated, and any deliberate deviations from the reference

---

## Build Mode

## Step 1: Understand the Request

Before writing any code, extract (or infer) these from the user's message:

- **Site type**: landing page, portfolio, business/company, web app
- **Purpose / audience**: who is this for, what should it achieve?
- **Content**: copy, sections needed, features, any brand details (name, colors, tone)
- **Special requirements**: forms, animations, dark/light mode, interactivity

If key info is missing and the user seems open to questions, ask 1–2 targeted questions. If the request is rich enough, proceed and make creative decisions.

## Step 2: Choose a Design Direction

Before writing code, commit to a **bold, specific aesthetic**. Do NOT default to generic:
- ❌ No purple gradients on white, no Inter/Roboto, no cookie-cutter layouts
- ✅ Pick an extreme: brutalist, editorial, art-deco, organic, luxury, playful, cinematic, retro-futuristic, etc.

Define:
- **Color palette**: 2–3 dominant colors + 1 sharp accent (use CSS variables)
- **Typography**: A distinctive display font (from Google Fonts CDN) + a refined body font. Make unexpected pairings.
- **Layout mood**: asymmetric, grid-breaking, generous whitespace, or controlled density
- **Motion**: plan 1–2 high-impact animations (page load reveal, scroll-triggered effects, hover states)

Every site should feel **custom-designed for its context** — no two sites should look alike.

## Step 3: Build the React Component

Output a **single self-contained `.jsx` file** using this structure:

```jsx
// Inline styles OR Tailwind utility classes (core only — no compiler needed)
// Google Fonts via <style> tag or @import in CSS-in-JS
// All components in one file, default export at bottom
```

### Component Architecture by Site Type

**Landing Page**
- Hero (headline, subheadline, CTA button, visual)
- Social proof / features / benefits section
- How it works or product showcase
- Testimonials or stats
- Final CTA / footer

**Portfolio / Personal Site**
- Hero with name, title, short bio
- Featured work / projects grid
- Skills or about section
- Contact section (email link or form)

**Business / Company Site**
- Hero with value proposition
- Services / offerings
- About / team section
- Trust signals (logos, stats, testimonials)
- Contact / CTA footer

**Web App**
- App shell with nav/sidebar
- Main feature UI (functional with useState/useReducer)
- Supporting panels/modals
- Responsive layout that works on mobile

### Code Quality Rules {#code-quality-rules}

- Use `useState`, `useEffect`, `useRef` for interactivity — make it actually work
- CSS-in-JS via inline `style` objects OR Tailwind core utilities (not both)
- All CSS variables defined at `:root` or in a top-level `<style>` tag injected via `useEffect`
- Responsive: use CSS media queries or Tailwind responsive prefixes
- Semantic HTML: `<header>`, `<main>`, `<section>`, `<footer>`, `aria-label` where needed
- No placeholder lorem ipsum — write real, purposeful copy for the site's context
- Images: use `https://picsum.photos` or `https://via.placeholder.com` for placeholders, or relevant SVG illustrations
- **Theme switcher**: Include for Build and Redesign modes (see [Theme Switcher](#theme-switcher))

### Theme Switcher {#theme-switcher}

Include a theme switcher with **system preference as default** when building or redesigning sites:

- **Options**: System (default), Light, Dark — System follows `prefers-color-scheme`
- **Implementation**: `useState` + `matchMedia('(prefers-color-scheme: light)')` for system; `localStorage` to persist user override; `data-theme` attribute or class for CSS variable overrides
- **Palette**: Define full light and dark palettes via CSS variables (e.g. `[data-theme="light"] { --bg: #f8f7f4; --text: #1a1917; ... }`)
- **UI**: Compact nav control (e.g. segmented buttons: System | Light | Dark); also include in mobile menu when nav collapses

### Animation Patterns

Prefer CSS-based animations for simplicity:

```jsx
// Fade-in on load example
const fadeInStyle = {
  animation: 'fadeIn 0.8s ease forwards',
};
// Inject keyframes via a <style> tag in useEffect
```

For scroll-triggered effects, use `IntersectionObserver` via `useEffect`.

## Step 4: Output

1. Write the complete `.jsx` file — no truncation, no `// ... rest of component`
2. Create `~/workspace/sites/dev/<site-name>/` with `index.html`, `App.jsx`, and optional `README.md`
3. Present the file to the user with `present_files`
4. In 2–3 sentences, describe the design direction you chose and any notable interactions

## Design Anti-Patterns to Avoid

- Generic hero with stock-photo background + centered white box
- Overused color combos: teal+white, purple+white, navy+gold (unless intentional subversion)
- Boring card grids with drop shadows as the only visual interest
- Default browser button styles with just a color change
- Symmetrical 3-column layouts for everything
- Animations that are purely decorative with no choreography

## Quick Reference: Font Pairings (use these as starting points, not defaults)

| Aesthetic | Display | Body |
|-----------|---------|------|
| Editorial | Playfair Display | DM Sans |
| Brutalist | Anton | IBM Plex Mono |
| Luxury | Cormorant Garamond | Jost |
| Playful | Righteous | Nunito |
| Tech/Minimal | Syne | Inter (acceptable here) |
| Organic | Fraunces | Source Serif 4 |
| Retro | Bebas Neue | Karla |

Always import from Google Fonts: `https://fonts.googleapis.com/css2?family=...`

## Reference Files

- `references/component-patterns.md` — reusable UI patterns (nav, hero variants, cards, forms)
- `references/animation-recipes.md` — copy-paste animation patterns for React

Read these if you need inspiration or a specific pattern implementation.