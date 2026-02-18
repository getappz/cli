/**
 * Branding runner using Cloudflare Browser Rendering (Playwright).
 * Loads URL in headless Chrome, executes branding script, returns raw branding data.
 */
import { launch } from "@cloudflare/playwright";
import { BRANDING_SCRIPT } from "../lib/branding/branding-script.inject";
/**
 * Create a fetcher that proxies requests to a local Browser Rendering emulator.
 * Use with runDevBrowser.mjs (pnpm dev:browser).
 */
export function createLocalBrowserFetcher(origin) {
    const base = origin.replace(/\/$/, "");
    return {
        fetch: (input, init) => {
            const request = input instanceof Request ? input : new Request(input, init);
            const url = new URL(request.url);
            const target = `${base}${url.pathname}${url.search}`;
            return fetch(target, {
                method: request.method,
                headers: request.headers,
                body: request.body,
            });
        },
    };
}
/**
 * Run branding extraction via Cloudflare Browser Rendering.
 * Launches headless Chrome, navigates to URL, runs branding script, returns result.
 */
export async function runBrandingExtraction(options) {
    const { browserBinding, url, timeout = 30000 } = options;
    let browser = null;
    try {
        browser = await launch(browserBinding);
        const page = await browser.newPage();
        await page.goto(url, {
            waitUntil: "domcontentloaded",
            timeout: Math.min(timeout, 25000),
        });
        // Run branding script in page context (IIFE returns { branding: {...} })
        // Pass script as arg—never embed in template literal (script contains ${} which would corrupt)
        const result = await page.evaluate((script) => {
            try {
                const fn = new Function("script", "return (0, eval)(script)");
                return fn(script);
            }
            catch (e) {
                return {
                    error: e instanceof Error ? e.message : String(e),
                };
            }
        }, BRANDING_SCRIPT);
        await browser.close();
        browser = null;
        if (result && typeof result === "object" && "error" in result) {
            return {
                success: false,
                error: result.error,
            };
        }
        const branding = result?.branding;
        if (!branding || typeof branding !== "object") {
            return {
                success: false,
                error: "Branding script returned invalid result",
            };
        }
        return {
            success: true,
            rawBranding: branding,
        };
    }
    catch (e) {
        if (browser) {
            try {
                await browser.close();
            }
            catch {
                // ignore
            }
        }
        return {
            success: false,
            error: e instanceof Error ? e.message : String(e),
        };
    }
}
//# sourceMappingURL=branding-runner.js.map