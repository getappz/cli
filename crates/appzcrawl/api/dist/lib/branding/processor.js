/**
 * Process raw branding script output into BrandingProfile.
 * Uses culori for color parsing ( Workers-compatible).
 */
import { formatHex, parse, rgb } from "culori";
export function hexify(rgba, background) {
    if (!rgba)
        return null;
    try {
        const color = parse(rgba);
        if (!color)
            return null;
        const rgbColor = rgb(color);
        if (!rgbColor || rgbColor.mode !== "rgb") {
            return null;
        }
        let r = Math.round((rgbColor.r ?? 0) * 255);
        let g = Math.round((rgbColor.g ?? 0) * 255);
        let b = Math.round((rgbColor.b ?? 0) * 255);
        const alpha = rgbColor.alpha ?? 1;
        if (alpha < 0.01) {
            return null;
        }
        if (alpha < 1) {
            let bgR = 255;
            let bgG = 255;
            let bgB = 255;
            if (background) {
                try {
                    const bgColor = parse(background);
                    if (bgColor) {
                        const bgRgb = rgb(bgColor);
                        if (bgRgb && bgRgb.mode === "rgb") {
                            const bgAlpha = bgRgb.alpha ?? 1;
                            if (bgAlpha >= 0.01) {
                                bgR = Math.round((bgRgb.r ?? 1) * 255);
                                bgG = Math.round((bgRgb.g ?? 1) * 255);
                                bgB = Math.round((bgRgb.b ?? 1) * 255);
                            }
                        }
                    }
                }
                catch {
                    // use white default
                }
            }
            r = Math.round(alpha * r + (1 - alpha) * bgR);
            g = Math.round(alpha * g + (1 - alpha) * bgG);
            b = Math.round(alpha * b + (1 - alpha) * bgB);
        }
        r = Math.max(0, Math.min(255, r));
        g = Math.max(0, Math.min(255, g));
        b = Math.max(0, Math.min(255, b));
        const hex = formatHex({ mode: "rgb", r: r / 255, g: g / 255, b: b / 255 });
        return hex ? hex.toUpperCase() : null;
    }
    catch {
        return null;
    }
}
function calculateRepresentativeBorderRadius(borderRadius) {
    if (!borderRadius)
        return "0px";
    const cornerValues = [
        borderRadius.topLeft || 0,
        borderRadius.topRight || 0,
        borderRadius.bottomRight || 0,
        borderRadius.bottomLeft || 0,
    ];
    const maxCorner = Math.max(...cornerValues);
    return maxCorner > 0 ? `${maxCorner}px` : "0px";
}
function contrastYIQ(hex) {
    if (!hex)
        return 0;
    const h = hex.replace("#", "");
    if (h.length < 6)
        return 0;
    const r = Number.parseInt(h.slice(0, 2), 16);
    const g = Number.parseInt(h.slice(2, 4), 16);
    const b = Number.parseInt(h.slice(4, 6), 16);
    return (r * 299 + g * 587 + b * 114) / 1000;
}
function inferPalette(snapshots, cssColors, colorScheme, pageBackground) {
    const freq = new Map();
    const bump = (hex, weight = 1) => {
        if (!hex)
            return;
        freq.set(hex, (freq.get(hex) || 0) + weight);
    };
    if (pageBackground) {
        const pageBgHex = hexify(pageBackground);
        if (pageBgHex)
            bump(pageBgHex, 1000);
    }
    for (const s of snapshots) {
        const area = Math.max(1, s.rect.w * s.rect.h);
        bump(hexify(s.colors.background, pageBackground), 0.5 + Math.log10(area + 10));
        bump(hexify(s.colors.text, pageBackground), 1.0);
        bump(hexify(s.colors.border, pageBackground), 0.3);
    }
    for (const c of cssColors)
        bump(hexify(c, pageBackground), 0.5);
    const ranked = Array.from(freq.entries())
        .sort((a, b) => b[1] - a[1])
        .map(([h]) => h);
    const isGrayish = (hex) => {
        const h = hex.replace("#", "");
        if (h.length < 6)
            return true;
        const r = Number.parseInt(h.slice(0, 2), 16);
        const g = Number.parseInt(h.slice(2, 4), 16);
        const b = Number.parseInt(h.slice(4, 6), 16);
        const max = Math.max(r, g, b);
        const min = Math.min(r, g, b);
        return max - min < 15;
    };
    let background = "#FFFFFF";
    if (pageBackground) {
        const pageBgHex = hexify(pageBackground);
        if (pageBgHex && isGrayish(pageBgHex)) {
            background = pageBgHex;
        }
    }
    if (background === "#FFFFFF" || (!pageBackground && ranked.length > 0)) {
        if (colorScheme === "dark") {
            background =
                ranked.find((h) => isGrayish(h) && contrastYIQ(h) < 128 && contrastYIQ(h) > 0) ||
                    ranked.find((h) => isGrayish(h) && contrastYIQ(h) < 180) ||
                    "#1A1A1A";
        }
        else {
            background =
                ranked.find((h) => isGrayish(h) && contrastYIQ(h) > 180) || "#FFFFFF";
        }
    }
    const textPrimary = ranked.find((h) => !/^#FFFFFF$/i.test(h) && contrastYIQ(h) < 160) ||
        (colorScheme === "dark" ? "#FFFFFF" : "#111111");
    const primary = ranked.find((h) => !isGrayish(h) && h !== textPrimary && h !== background) || (colorScheme === "dark" ? "#FFFFFF" : "#000000");
    const accent = ranked.find((h) => h !== primary && !isGrayish(h)) || primary;
    return {
        primary,
        accent,
        background,
        textPrimary,
        link: accent,
    };
}
function inferBaseUnit(values) {
    const vs = values
        .filter((v) => Number.isFinite(v) && v > 0 && v <= 128)
        .map((v) => Math.round(v));
    if (vs.length === 0)
        return 8;
    const candidates = [4, 6, 8, 10, 12];
    for (const c of candidates) {
        const ok = vs.filter((v) => v % c === 0 || Math.abs((v % c) - c) <= 1 || v % c <= 1)
            .length / vs.length;
        if (ok >= 0.6)
            return c;
    }
    vs.sort((a, b) => a - b);
    const med = vs[Math.floor(vs.length / 2)];
    return Math.max(2, Math.min(12, Math.round(med / 2) * 2));
}
function pickBorderRadius(radii) {
    const rs = radii.filter((v) => Number.isFinite(v));
    if (!rs.length)
        return "8px";
    rs.sort((a, b) => a - b);
    const med = rs[Math.floor(rs.length / 2)];
    return `${Math.round(med)}px`;
}
function inferFontsList(fontStacks) {
    const freq = {};
    for (const stack of fontStacks) {
        for (const f of stack) {
            if (f)
                freq[f] = (freq[f] || 0) + 1;
        }
    }
    return Object.keys(freq)
        .sort((a, b) => freq[b] - freq[a])
        .slice(0, 10)
        .map((f) => ({ family: f, count: freq[f] }));
}
function pickLogo(images) {
    const byType = (t) => images.find((i) => i.type === t)?.src;
    return byType("logo") || byType("logo-svg") || null;
}
function extractInputSnapshots(raw, _uniqueButtons) {
    const candidateInputs = raw.snapshots.filter((s) => {
        if (!s.isInput || !s.inputMetadata)
            return false;
        if (s.rect.w < 50 || s.rect.h < 20)
            return false;
        return true;
    });
    const scoredInputs = candidateInputs
        .map((input, idx) => {
        if (!input.inputMetadata)
            return null;
        const meta = input.inputMetadata;
        let score = 0;
        if (meta.type === "email")
            score += 100;
        else if (meta.type === "text")
            score += 80;
        else if (meta.type === "password")
            score += 70;
        else if (meta.type === "search")
            score += 60;
        else if (meta.type === "tel")
            score += 50;
        else if (meta.type === "textarea")
            score += 40;
        else if (meta.type === "select")
            score += 30;
        if (meta.required)
            score += 50;
        if (meta.placeholder)
            score += 30;
        if (meta.label)
            score += 40;
        const allText = `${meta.placeholder} ${meta.label} ${meta.name}`.toLowerCase();
        if (allText.includes("email"))
            score += 80;
        if (allText.includes("search"))
            score += 60;
        if (allText.includes("password"))
            score += 50;
        if (allText.includes("name"))
            score += 40;
        return { input, score, idx };
    })
        .filter((item) => item !== null);
    scoredInputs.sort((a, b) => b.score - a.score);
    const topInputs = scoredInputs.slice(0, 20);
    const results = [];
    for (const { input } of topInputs) {
        if (!input.inputMetadata)
            continue;
        const meta = input.inputMetadata;
        const bgHex = hexify(input.colors.background, raw.pageBackground);
        const borderHex = input.colors.borderWidth && input.colors.borderWidth > 0
            ? hexify(input.colors.border, raw.pageBackground)
            : null;
        const corners = {
            topLeft: input.borderRadius?.topLeft != null
                ? `${input.borderRadius.topLeft}px`
                : "0px",
            topRight: input.borderRadius?.topRight != null
                ? `${input.borderRadius.topRight}px`
                : "0px",
            bottomRight: input.borderRadius?.bottomRight != null
                ? `${input.borderRadius.bottomRight}px`
                : "0px",
            bottomLeft: input.borderRadius?.bottomLeft != null
                ? `${input.borderRadius.bottomLeft}px`
                : "0px",
        };
        const representativeBorderRadius = calculateRepresentativeBorderRadius(input.borderRadius);
        results.push({
            type: meta.type,
            placeholder: meta.placeholder,
            label: meta.label,
            name: meta.name,
            required: meta.required,
            classes: input.classes ?? "",
            background: bgHex || "transparent",
            textColor: hexify(input.colors.text, raw.pageBackground),
            borderColor: borderHex ?? undefined,
            borderRadius: representativeBorderRadius,
            borderRadiusCorners: corners,
            shadow: input.shadow ?? undefined,
        });
    }
    return results;
}
export function processRawBranding(raw) {
    const palette = inferPalette(raw.snapshots, raw.cssData.colors, raw.colorScheme, raw.pageBackground);
    const typography = {
        fontFamilies: {
            primary: raw.typography.stacks.body[0] || "system-ui, sans-serif",
            heading: raw.typography.stacks.heading[0] ||
                raw.typography.stacks.body[0] ||
                "system-ui, sans-serif",
        },
        fontStacks: raw.typography.stacks,
        fontSizes: raw.typography.sizes,
    };
    const baseUnit = inferBaseUnit(raw.cssData.spacings);
    const borderRadius = pickBorderRadius([
        ...raw.snapshots.map((s) => s.radius),
        ...raw.cssData.radii,
    ]);
    const allFontStacks = [
        ...Object.values(raw.typography.stacks).flat(),
        ...raw.snapshots.flatMap((s) => s.typography.fontStack),
    ];
    const fontsList = inferFontsList([allFontStacks]);
    const images = {
        logo: pickLogo(raw.images),
        favicon: raw.images.find((i) => i.type === "favicon")?.src ?? null,
        ogImage: raw.images.find((i) => i.type === "og")?.src ||
            raw.images.find((i) => i.type === "twitter")?.src ||
            null,
    };
    const components = {};
    const candidateButtons = raw.snapshots
        .filter((s) => {
        if (!s.isButton)
            return false;
        if (s.rect.w < 30 || s.rect.h < 30)
            return false;
        if (!s.text || s.text.trim().length === 0)
            return false;
        const bgHex = hexify(s.colors.background, raw.pageBackground);
        const hasBorder = s.colors.borderWidth && s.colors.borderWidth > 0;
        const _borderHex = hasBorder
            ? hexify(s.colors.border, raw.pageBackground)
            : null;
        if (!bgHex && !hasBorder)
            return false;
        return true;
    })
        .map((s) => {
        let score = 0;
        if (s.hasCTAIndicator)
            score += 1000;
        const text = (s.text || "").toLowerCase();
        const ctaKeywords = [
            "sign up",
            "get started",
            "start deploying",
            "start",
            "deploy",
            "try",
            "demo",
            "contact",
            "buy",
            "subscribe",
            "join",
            "register",
            "get",
            "free",
        ];
        if (ctaKeywords.some((kw) => text.includes(kw)))
            score += 500;
        const bgHex = hexify(s.colors.background, raw.pageBackground);
        const borderHex = s.colors.borderWidth && s.colors.borderWidth > 0
            ? hexify(s.colors.border, raw.pageBackground)
            : null;
        if (bgHex &&
            bgHex !== "#FFFFFF" &&
            bgHex !== "#FAFAFA" &&
            bgHex !== "#F5F5F5") {
            score += 300;
        }
        if (borderHex && !bgHex)
            score += 200;
        if (text.length > 0 && text.length < 50)
            score += 100;
        const area = (s.rect.w || 0) * (s.rect.h || 0);
        score += Math.log10(area + 1) * 10;
        return { ...s, _score: score };
    })
        .sort((a, b) => (b._score || 0) - (a._score || 0));
    const seenButtons = new Map();
    const uniqueButtons = [];
    for (const button of candidateButtons) {
        const bgHex = hexify(button.colors.background, raw.pageBackground) || "transparent";
        const borderHex = button.colors.borderWidth && button.colors.borderWidth > 0
            ? hexify(button.colors.border, raw.pageBackground) ||
                "transparent-border"
            : "no-border";
        const textKey = (button.text || "").trim().toLowerCase().substring(0, 50);
        const classKey = (button.classes || "")
            .split(/\s+/)
            .slice(0, 5)
            .join(" ")
            .toLowerCase();
        const signature = `${textKey}|${bgHex}|${borderHex}|${classKey}`;
        if (!seenButtons.has(signature)) {
            seenButtons.set(signature, 1);
            uniqueButtons.push(button);
        }
        else {
            seenButtons.set(signature, (seenButtons.get(signature) ?? 0) + 1);
        }
    }
    const topButtons = uniqueButtons.slice(0, 80);
    const buttonSnapshots = topButtons.map((s, idx) => {
        let bgHex = hexify(s.colors.background, raw.pageBackground);
        const borderHex = s.colors.borderWidth && s.colors.borderWidth > 0
            ? hexify(s.colors.border, raw.pageBackground)
            : null;
        if (!bgHex)
            bgHex = "transparent";
        const corners = {
            topLeft: s.borderRadius?.topLeft != null ? `${s.borderRadius.topLeft}px` : "0px",
            topRight: s.borderRadius?.topRight != null
                ? `${s.borderRadius.topRight}px`
                : "0px",
            bottomRight: s.borderRadius?.bottomRight != null
                ? `${s.borderRadius.bottomRight}px`
                : "0px",
            bottomLeft: s.borderRadius?.bottomLeft != null
                ? `${s.borderRadius.bottomLeft}px`
                : "0px",
        };
        const representativeBorderRadius = calculateRepresentativeBorderRadius(s.borderRadius);
        return {
            index: idx,
            text: s.text || "",
            html: "",
            classes: s.classes || "",
            background: bgHex,
            textColor: hexify(s.colors.text, raw.pageBackground) || "#000000",
            borderColor: borderHex,
            borderRadius: representativeBorderRadius,
            borderRadiusCorners: corners,
            shadow: s.shadow ?? null,
            originalBackgroundColor: s.colors.background || undefined,
            originalTextColor: s.colors.text || undefined,
            originalBorderColor: s.colors.border || undefined,
        };
    });
    const inputSnapshots = extractInputSnapshots(raw, uniqueButtons);
    if (inputSnapshots.length > 0) {
        const primaryInput = inputSnapshots[0];
        components.input = {
            background: primaryInput.background,
            textColor: primaryInput.textColor,
            borderColor: primaryInput.borderColor,
            borderRadius: primaryInput.borderRadius,
            borderRadiusCorners: primaryInput.borderRadiusCorners,
            shadow: primaryInput.shadow,
        };
    }
    return {
        colorScheme: raw.colorScheme,
        fonts: fontsList,
        colors: palette,
        typography,
        spacing: {
            baseUnit,
            borderRadius,
        },
        components,
        images,
        __button_snapshots: buttonSnapshots,
        __input_snapshots: inputSnapshots,
        __framework_hints: raw.frameworkHints,
        __logo_candidates: raw.logoCandidates,
    };
}
//# sourceMappingURL=processor.js.map