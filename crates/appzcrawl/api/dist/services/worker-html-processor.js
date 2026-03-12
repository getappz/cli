/**
 * Worker-native HTML processing using Cloudflare's HTMLRewriter.
 *
 * Zero external dependencies — uses only APIs available in the Workers runtime.
 * Each function matches the signature/return shape of its native-container.ts
 * counterpart so the two backends are interchangeable.
 *
 * HTMLRewriter is powered by lol_html (the same Rust library the container
 * uses), so behaviour should be nearly identical for tag/attribute extraction.
 */
// ---------------------------------------------------------------------------
// Internal helper: drain an HTMLRewriter pipeline to trigger all handlers.
// ---------------------------------------------------------------------------
async function drain(html, setup) {
    const rw = setup(new HTMLRewriter());
    // .text() drains the response stream, running all element/text handlers.
    await rw.transform(new Response(html)).text();
}
// ---------------------------------------------------------------------------
// Resolve a potentially-relative URL against a base URL.
// ---------------------------------------------------------------------------
function resolveUrl(src, base) {
    if (!src ||
        src.startsWith("data:") ||
        src.startsWith("blob:") ||
        src.startsWith("javascript:")) {
        return src;
    }
    try {
        return new URL(src, base).href;
    }
    catch {
        return src;
    }
}
// =========================================================================
// 1. extractLinks
// =========================================================================
export async function extractLinks(html) {
    if (!html)
        return { links: [] };
    const links = [];
    await drain(html, (rw) => rw.on("a[href]", {
        element(el) {
            const href = el.getAttribute("href");
            if (href)
                links.push(href);
        },
    }));
    return { links };
}
// =========================================================================
// 2. extractBaseHref
// =========================================================================
export async function extractBaseHref(html, baseUrl) {
    let baseHref = "";
    await drain(html, (rw) => rw.on("base[href]", {
        element(el) {
            if (baseHref)
                return; // only take first
            const href = el.getAttribute("href");
            if (href) {
                try {
                    baseHref = new URL(href, baseUrl).href;
                }
                catch {
                    baseHref = href;
                }
            }
        },
    }));
    return { baseHref };
}
// =========================================================================
// 3. extractMetadata
// =========================================================================
export async function extractMetadata(html) {
    if (!html)
        return { metadata: {} };
    const metadata = {};
    let titleText = "";
    let capturingTitle = false;
    await drain(html, (rw) => rw
        // <html lang="…">
        .on("html", {
        element(el) {
            const lang = el.getAttribute("lang");
            if (lang)
                metadata.language = lang;
        },
    })
        // <title>…</title>
        .on("title", {
        element() {
            capturingTitle = true;
            titleText = "";
        },
        text(chunk) {
            if (capturingTitle)
                titleText += chunk.text;
            if (chunk.lastInTextNode)
                capturingTitle = false;
        },
    })
        // <meta …>
        .on("meta", {
        element(el) {
            const name = (el.getAttribute("name") ?? "").toLowerCase();
            const property = (el.getAttribute("property") ?? "").toLowerCase();
            const content = el.getAttribute("content") ?? "";
            const charset = el.getAttribute("charset");
            if (charset)
                metadata.charset = charset;
            // Standard
            if (name === "description")
                metadata.description = content;
            if (name === "keywords")
                metadata.keywords = content;
            if (name === "author")
                metadata.author = content;
            if (name === "robots")
                metadata.robots = content;
            if (name === "generator")
                metadata.generator = content;
            // OpenGraph
            if (property === "og:title")
                metadata.ogTitle = content;
            if (property === "og:description")
                metadata.ogDescription = content;
            if (property === "og:image")
                metadata.ogImage = content;
            if (property === "og:url")
                metadata.ogUrl = content;
            if (property === "og:type")
                metadata.ogType = content;
            if (property === "og:site_name")
                metadata.ogSiteName = content;
            if (property === "og:locale")
                metadata.ogLocale = content;
            // Twitter
            const twitterKey = name.startsWith("twitter:")
                ? name
                : property.startsWith("twitter:")
                    ? property
                    : "";
            if (twitterKey) {
                const key = twitterKey.replace("twitter:", "");
                if (!metadata.twitter)
                    metadata.twitter = {};
                metadata.twitter[key] = content;
            }
            // Dublin Core
            if (name.startsWith("dc.") || name.startsWith("dcterms.")) {
                if (!metadata.dublinCore)
                    metadata.dublinCore = {};
                metadata.dublinCore[name] = content;
            }
        },
    })
        // <link rel="canonical" href="…">
        .on('link[rel="canonical"]', {
        element(el) {
            const href = el.getAttribute("href");
            if (href)
                metadata.canonical = href;
        },
    })
        // Favicon
        .on('link[rel="icon"], link[rel="shortcut icon"]', {
        element(el) {
            const href = el.getAttribute("href");
            if (href && !metadata.favicon)
                metadata.favicon = href;
        },
    }));
    if (titleText.trim())
        metadata.title = titleText.trim();
    return { metadata };
}
// =========================================================================
// 4. getInnerJson  (extracts body text content — matches Rust behaviour)
// =========================================================================
export async function getInnerJson(html) {
    let content = "";
    let insideBody = false;
    // Track depth to skip <script> and <style> within body
    let skipDepth = 0;
    await drain(html, (rw) => rw
        .on("body", {
        element() {
            insideBody = true;
        },
    })
        .on("script, style, noscript", {
        element(el) {
            if (insideBody) {
                skipDepth++;
                el.onEndTag(() => {
                    skipDepth--;
                });
            }
        },
    })
        .on("body *", {
        text(chunk) {
            if (insideBody && skipDepth === 0) {
                content += chunk.text;
            }
        },
    }));
    return { content: content.trim() };
}
// =========================================================================
// 5. extractImages
// =========================================================================
export async function extractImages(html, baseUrl) {
    const imageSet = new Set();
    // Determine effective base for relative URL resolution
    let effectiveBase = baseUrl;
    // Quick pre-scan for <base href>
    const baseResult = await extractBaseHref(html, baseUrl);
    if (baseResult.baseHref)
        effectiveBase = baseResult.baseHref;
    const addImage = (src) => {
        if (!src)
            return;
        const resolved = resolveUrl(src.trim(), effectiveBase);
        if (resolved &&
            !resolved.startsWith("javascript:") &&
            resolved.length > 0) {
            imageSet.add(resolved);
        }
    };
    /** Parse srcset attribute into individual URLs. */
    const parseSrcset = (srcset) => {
        for (const part of srcset.split(",")) {
            const url = part.trim().split(/\s+/)[0];
            if (url)
                addImage(url);
        }
    };
    await drain(html, (rw) => rw
        // <img src="…" data-src="…" srcset="…">
        .on("img", {
        element(el) {
            addImage(el.getAttribute("src"));
            addImage(el.getAttribute("data-src"));
            const srcset = el.getAttribute("srcset");
            if (srcset)
                parseSrcset(srcset);
        },
    })
        // <picture><source srcset="…">
        .on("picture source", {
        element(el) {
            const srcset = el.getAttribute("srcset");
            if (srcset)
                parseSrcset(srcset);
        },
    })
        // OG / Twitter / itemprop images
        .on('meta[property="og:image"], meta[property="og:image:url"], meta[property="og:image:secure_url"]', {
        element(el) {
            addImage(el.getAttribute("content"));
        },
    })
        .on('meta[name="twitter:image"], meta[name="twitter:image:src"], meta[itemprop="image"]', {
        element(el) {
            addImage(el.getAttribute("content"));
        },
    })
        // Icons
        .on("link[rel*='icon'], link[rel*='apple-touch-icon'], link[rel*='image_src']", {
        element(el) {
            addImage(el.getAttribute("href"));
        },
    })
        // <video poster="…">
        .on("video[poster]", {
        element(el) {
            addImage(el.getAttribute("poster"));
        },
    })
        // Inline background-image via style attribute
        .on("[style]", {
        element(el) {
            const style = el.getAttribute("style") ?? "";
            if (style.includes("background")) {
                const urlPattern = /url\(\s*['"]?([^'")]+)['"]?\s*\)/gi;
                let match;
                while ((match = urlPattern.exec(style)) !== null) {
                    addImage(match[1]);
                }
            }
        },
    }));
    return { images: Array.from(imageSet) };
}
// =========================================================================
// 6. extractAssets
// =========================================================================
export async function extractAssets(html, baseUrl, formats) {
    const requested = new Set(formats ?? ["assets"]);
    const wantsAll = requested.has("assets");
    const images = [];
    const css = [];
    const js = [];
    const fonts = [];
    const videos = [];
    const audio = [];
    const iframes = [];
    // Determine effective base
    let effectiveBase = baseUrl;
    const baseResult = await extractBaseHref(html, baseUrl);
    if (baseResult.baseHref)
        effectiveBase = baseResult.baseHref;
    const resolve = (src) => {
        if (!src)
            return "";
        return resolveUrl(src.trim(), effectiveBase);
    };
    await drain(html, (rw) => {
        let r = rw;
        // Images
        if (wantsAll || requested.has("images")) {
            r = r.on("img[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        images.push(v);
                },
            });
        }
        // CSS
        if (wantsAll || requested.has("css")) {
            r = r.on('link[rel="stylesheet"]', {
                element(el) {
                    const v = resolve(el.getAttribute("href"));
                    if (v)
                        css.push(v);
                },
            });
        }
        // JS
        if (wantsAll || requested.has("js")) {
            r = r.on("script[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        js.push(v);
                },
            });
        }
        // Fonts  (link[rel="preload"][as="font"] and link[rel="stylesheet"] with font in href)
        if (wantsAll || requested.has("fonts")) {
            r = r.on('link[rel="preload"][as="font"], link[as="font"]', {
                element(el) {
                    const v = resolve(el.getAttribute("href"));
                    if (v)
                        fonts.push(v);
                },
            });
        }
        // Videos
        if (wantsAll || requested.has("videos")) {
            r = r
                .on("video[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        videos.push(v);
                },
            })
                .on("video source[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        videos.push(v);
                },
            });
        }
        // Audio
        if (wantsAll || requested.has("audio")) {
            r = r
                .on("audio[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        audio.push(v);
                },
            })
                .on("audio source[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        audio.push(v);
                },
            });
        }
        // Iframes
        if (wantsAll || requested.has("iframes")) {
            r = r.on("iframe[src]", {
                element(el) {
                    const v = resolve(el.getAttribute("src"));
                    if (v)
                        iframes.push(v);
                },
            });
        }
        return r;
    });
    return { images, css, js, fonts, videos, audio, iframes };
}
// =========================================================================
// 7. postProcessMarkdown  (matches Rust: fix multi-line links + skip-links)
// =========================================================================
export function postProcessMarkdown(markdown, _options) {
    // 1. Fix multi-line links: inside [...] newlines become `\<newline>`
    let linkOpenCount = 0;
    let out = "";
    for (const ch of markdown) {
        if (ch === "[")
            linkOpenCount++;
        else if (ch === "]")
            linkOpenCount = Math.max(0, linkOpenCount - 1);
        if (linkOpenCount > 0 && ch === "\n") {
            out += "\\\n";
        }
        else {
            out += ch;
        }
    }
    // 2. Remove "Skip to Content" links  [Skip to Content](#…)
    out = out.replace(/\[skip to content\]\(#[^)]*\)/gi, "");
    return { markdown: out };
}
// =========================================================================
// 8. parseSitemap  (simple XML extraction — matches Rust quick-xml output)
// =========================================================================
export function parseSitemap(xml) {
    const urls = [];
    const sitemapUrls = [];
    // <sitemap>…<loc>…</loc>…</sitemap>
    // <url>…<loc>…</loc>…</url>
    // Since sitemap XML is very regular, regex is reliable and avoids
    // pulling in an XML parser dependency.
    // Extract <sitemap><loc>…</loc></sitemap>
    const sitemapBlockRe = /<sitemap[^>]*>[\s\S]*?<\/sitemap>/gi;
    const locRe = /<loc>\s*<!\[CDATA\[([\s\S]*?)\]\]>\s*<\/loc>|<loc>\s*([\s\S]*?)\s*<\/loc>/gi;
    // First pass: extract sitemap URLs
    let sitemapMatch;
    while ((sitemapMatch = sitemapBlockRe.exec(xml)) !== null) {
        let locMatch;
        locRe.lastIndex = 0;
        while ((locMatch = locRe.exec(sitemapMatch[0])) !== null) {
            const loc = (locMatch[1] ?? locMatch[2] ?? "").trim();
            if (loc)
                sitemapUrls.push(loc);
        }
    }
    // Second pass: extract page URLs from <url> blocks
    const urlBlockRe = /<url[^>]*>[\s\S]*?<\/url>/gi;
    let urlMatch;
    while ((urlMatch = urlBlockRe.exec(xml)) !== null) {
        let locMatch;
        locRe.lastIndex = 0;
        while ((locMatch = locRe.exec(urlMatch[0])) !== null) {
            const loc = (locMatch[1] ?? locMatch[2] ?? "").trim();
            if (loc)
                urls.push(loc);
        }
    }
    return { urls, sitemapUrls };
}
// =========================================================================
// 9. transformHtml  (strip tags, exclude non-main, resolve URLs)
// =========================================================================
/**
 * Selectors removed when `only_main_content` is true.
 * Mirrors the Rust EXCLUDE_NON_MAIN_TAGS list in firecrawl html.rs.
 */
const EXCLUDE_NON_MAIN_SELECTORS = [
    "header",
    "footer",
    "nav",
    "aside",
    ".header",
    ".top",
    ".navbar",
    "#header",
    ".footer",
    ".bottom",
    "#footer",
    ".sidebar",
    ".side",
    ".aside",
    "#sidebar",
    ".modal",
    ".popup",
    "#modal",
    ".overlay",
    ".ad",
    ".ads",
    ".advert",
    "#ad",
    ".lang-selector",
    ".language",
    "#language-selector",
    ".social",
    ".social-media",
    ".social-links",
    "#social",
    ".menu",
    ".navigation",
    "#nav",
    ".breadcrumbs",
    "#breadcrumbs",
    ".share",
    "#share",
    ".widget",
    "#widget",
    ".cookie",
    "#cookie",
    ".fc-decoration",
];
/** Tags always stripped regardless of options. */
const ALWAYS_STRIP_TAGS = ["script", "style", "noscript"];
/**
 * Returns `true` when the Worker-native implementation can handle the
 * given params.  Falls back to the container for features that require
 * DOM-tree operations (include_tags, OMCE signatures).
 */
export function canHandleTransformHtml(params) {
    // include_tags requires "keep only these" — needs DOM tree, not streaming
    if (params.include_tags && params.include_tags.length > 0)
        return false;
    // OMCE signature matching requires tree traversal with node signatures
    if (params.omce_signatures && params.omce_signatures.length > 0)
        return false;
    return true;
}
/**
 * Transform and clean HTML using HTMLRewriter.
 *
 * What it does (matching the Rust implementation):
 * 1. Strip `<head>`, `<meta>`, `<script>`, `<style>`, `<noscript>`
 * 2. Apply `exclude_tags` removals
 * 3. If `only_main_content`: remove 42 non-main selectors (nav, header,
 *    footer, sidebar, ads, social, cookie banners, etc.)
 * 4. Resolve relative `<img src>` and `<a href>` to absolute URLs
 * 5. Pick largest srcset image as the `src`
 *
 * Does NOT handle (falls back to container via `canHandleTransformHtml`):
 * - `include_tags` (needs DOM tree to "keep only matching elements")
 * - OMCE signature matching (needs tree traversal)
 */
export async function transformHtml(params) {
    const { html, url } = params;
    const excludeTags = params.exclude_tags ?? [];
    const onlyMainContent = params.only_main_content ?? false;
    // Determine the base URL for resolving relative links
    let baseUrl = url;
    try {
        const baseResult = await extractBaseHref(html, url);
        if (baseResult.baseHref)
            baseUrl = baseResult.baseHref;
    }
    catch {
        // ignore
    }
    // Build the full list of selectors to remove
    const selectorsToRemove = [...ALWAYS_STRIP_TAGS, "head", ...excludeTags];
    if (onlyMainContent) {
        selectorsToRemove.push(...EXCLUDE_NON_MAIN_SELECTORS);
    }
    // Build a single HTMLRewriter with all handlers
    let rw = new HTMLRewriter();
    // Remove matching elements
    for (const selector of selectorsToRemove) {
        rw = rw.on(selector, {
            element(el) {
                el.remove();
            },
        });
    }
    // Remove <meta> tags (strip them from output like Rust does)
    rw = rw.on("meta", {
        element(el) {
            el.remove();
        },
    });
    // Resolve relative img src to absolute + pick best srcset
    rw = rw.on("img", {
        element(el) {
            // Resolve src
            const src = el.getAttribute("src");
            if (src) {
                const resolved = resolveUrl(src, baseUrl);
                if (resolved !== src)
                    el.setAttribute("src", resolved);
            }
            // Pick best srcset image
            const srcset = el.getAttribute("srcset");
            if (srcset) {
                let bestUrl = "";
                let bestSize = -1;
                for (const part of srcset.split(",")) {
                    const tokens = part.trim().split(/\s+/);
                    const candidateUrl = tokens[0];
                    if (!candidateUrl)
                        continue;
                    let size = 1;
                    const descriptor = tokens[tokens.length - 1];
                    if (tokens.length > 1 &&
                        descriptor &&
                        (descriptor.endsWith("w") || descriptor.endsWith("x"))) {
                        const parsed = Number.parseFloat(descriptor);
                        if (!Number.isNaN(parsed))
                            size = parsed;
                    }
                    if (size > bestSize) {
                        bestSize = size;
                        bestUrl = candidateUrl;
                    }
                }
                if (bestUrl) {
                    const resolved = resolveUrl(bestUrl, baseUrl);
                    el.setAttribute("src", resolved);
                }
                el.removeAttribute("srcset");
            }
            // Resolve data-src
            const dataSrc = el.getAttribute("data-src");
            if (dataSrc) {
                const resolved = resolveUrl(dataSrc, baseUrl);
                if (resolved !== dataSrc)
                    el.setAttribute("data-src", resolved);
            }
        },
    });
    // Resolve relative a href to absolute
    rw = rw.on("a[href]", {
        element(el) {
            const href = el.getAttribute("href");
            if (href) {
                const resolved = resolveUrl(href, baseUrl);
                if (resolved !== href)
                    el.setAttribute("href", resolved);
            }
        },
    });
    const transformed = await rw.transform(new Response(html)).text();
    return { html: transformed };
}
//# sourceMappingURL=worker-html-processor.js.map