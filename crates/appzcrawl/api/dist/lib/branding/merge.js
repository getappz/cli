/**
 * Merge JS analysis with LLM enhancement (or heuristic-only) into final BrandingProfile.
 */
import { logger } from "../logger";
import { calculateLogoArea } from "./types";
export function mergeBrandingResults(js, llm, buttonSnapshots, logoCandidates) {
    const merged = { ...js };
    const hasLogoCandidates = !!(logoCandidates && logoCandidates.length > 0);
    if (hasLogoCandidates &&
        llm.logoSelection &&
        llm.logoSelection.selectedLogoIndex !== undefined) {
        if (llm.logoSelection.selectedLogoIndex === -1) {
            if (merged.images) {
                delete merged.images.logo;
                delete merged.images.logoHref;
                delete merged.images.logoAlt;
            }
            merged.__llm_logo_reasoning = {
                selectedIndex: -1,
                reasoning: llm.logoSelection.selectedLogoReasoning || "No valid logo found",
                confidence: llm.logoSelection.confidence || 0,
                rejected: true,
                source: "llm",
            };
        }
        else if (llm.logoSelection.selectedLogoIndex >= 0 &&
            logoCandidates &&
            logoCandidates.length > 0 &&
            llm.logoSelection.selectedLogoIndex < logoCandidates.length) {
            const selectedLogo = logoCandidates[llm.logoSelection.selectedLogoIndex];
            if (selectedLogo) {
                const confidence = llm.logoSelection.confidence || 0;
                const alt = selectedLogo.alt || "";
                const altLower = alt.toLowerCase().trim();
                const href = selectedLogo.href || "";
                const isLanguageWord = /^(english|español|français|deutsch|italiano|português|中文|日本語|한국어|русский|العربية|en|es|fr|de|it|pt|zh|ja|ko|ru|ar)$/i.test(altLower);
                const isCommonMenuWord = /^(menu|search|cart|login|signup|register|account|profile|settings|help|support|contact|about|home|shop|store|products|services|blog|news)$/i.test(altLower);
                const isUIIcon = /search|icon|menu|hamburger|cart|user|bell|notification|settings|close|times/i.test(altLower);
                let isExternalLink = false;
                if (href?.trim()) {
                    const hrefLower = href.toLowerCase().trim();
                    if (hrefLower.startsWith("http://") ||
                        hrefLower.startsWith("https://") ||
                        hrefLower.startsWith("//")) {
                        const externalServiceDomains = [
                            "github.com",
                            "twitter.com",
                            "x.com",
                            "facebook.com",
                            "linkedin.com",
                        ];
                        if (externalServiceDomains.some((domain) => hrefLower.includes(domain))) {
                            isExternalLink = true;
                        }
                    }
                }
                const width = selectedLogo.position?.width || 0;
                const height = selectedLogo.position?.height || 0;
                const isSmallSquareIcon = Math.abs(width - height) < 5 && width < 40 && width > 0;
                const trustLLMForLogo = confidence >= 0.7;
                const smallSquareIconLikelyUi = isSmallSquareIcon &&
                    !(trustLLMForLogo && selectedLogo.indicators?.inHeader);
                const area = calculateLogoArea(selectedLogo.position);
                const hasReasonableSize = area >= 500 && area <= 100000;
                const hasStrongIndicators = selectedLogo.indicators?.inHeader &&
                    selectedLogo.indicators?.hrefMatch &&
                    hasReasonableSize;
                const reasoning = llm.logoSelection.selectedLogoReasoning ?? "";
                const isHeuristicOrFallback = reasoning.includes("Heuristic") ||
                    reasoning.includes("heuristic") ||
                    reasoning === "LLM failed" ||
                    reasoning.includes("invalid index");
                const smallSquareRedFlag = smallSquareIconLikelyUi &&
                    !hasStrongIndicators &&
                    isHeuristicOrFallback;
                const hasRedFlagsWithLLMTrust = isLanguageWord ||
                    isCommonMenuWord ||
                    isUIIcon ||
                    smallSquareRedFlag ||
                    isExternalLink;
                const shouldIncludeLogoWithLLMTrust = !hasRedFlagsWithLLMTrust &&
                    (confidence >= 0.5 || (hasStrongIndicators && confidence >= 0.4));
                if (shouldIncludeLogoWithLLMTrust) {
                    if (!merged.images) {
                        merged.images = {};
                    }
                    merged.images.logo = selectedLogo.src;
                    if (selectedLogo.href) {
                        merged.images.logoHref = selectedLogo.href;
                    }
                    else {
                        delete merged.images.logoHref;
                    }
                    if (selectedLogo.alt) {
                        merged.images.logoAlt = selectedLogo.alt;
                    }
                    else {
                        delete merged.images.logoAlt;
                    }
                    merged.__llm_logo_reasoning = {
                        selectedIndex: llm.logoSelection.selectedLogoIndex,
                        reasoning: llm.logoSelection.selectedLogoReasoning,
                        confidence: llm.logoSelection.confidence,
                        source: isHeuristicOrFallback ? "heuristic" : "llm",
                    };
                    logger.debug("[branding merge] Logo included", {
                        result: "included",
                        selectedIndex: llm.logoSelection.selectedLogoIndex,
                        source: isHeuristicOrFallback ? "heuristic" : "llm",
                        confidence,
                        reasoning: (reasoning || "").slice(0, 120),
                    });
                }
                else {
                    let rejectionReason = "Low confidence";
                    const redFlagReasons = [];
                    if (hasRedFlagsWithLLMTrust) {
                        if (isLanguageWord)
                            redFlagReasons.push("language word");
                        if (isCommonMenuWord)
                            redFlagReasons.push("menu word");
                        if (isUIIcon)
                            redFlagReasons.push("UI icon");
                        if (smallSquareRedFlag)
                            redFlagReasons.push("small square icon");
                        if (isExternalLink)
                            redFlagReasons.push("external link");
                        rejectionReason = `Red flags detected (${redFlagReasons.join(", ")})`;
                    }
                    const selectedLogoReasoning = reasoning.trim();
                    merged.__llm_logo_reasoning = {
                        selectedIndex: llm.logoSelection.selectedLogoIndex,
                        reasoning: selectedLogoReasoning
                            ? `Logo rejected: ${rejectionReason}. ${selectedLogoReasoning}`
                            : `Logo rejected: ${rejectionReason}.`,
                        confidence: llm.logoSelection.confidence,
                        rejected: true,
                        source: isHeuristicOrFallback ? "heuristic" : "llm",
                    };
                }
            }
        }
    }
    if (buttonSnapshots.length > 0) {
        const primaryIdx = llm.buttonClassification.primaryButtonIndex;
        const secondaryIdx = llm.buttonClassification.secondaryButtonIndex;
        merged.__llm_button_reasoning = {
            primary: {
                index: primaryIdx,
                text: primaryIdx >= 0 ? buttonSnapshots[primaryIdx]?.text : "N/A",
                reasoning: llm.buttonClassification.primaryButtonReasoning,
            },
            secondary: {
                index: secondaryIdx,
                text: secondaryIdx >= 0 ? buttonSnapshots[secondaryIdx]?.text : "N/A",
                reasoning: llm.buttonClassification.secondaryButtonReasoning,
            },
            confidence: llm.buttonClassification.confidence,
        };
    }
    if (llm.buttonClassification.confidence > 0.5 && buttonSnapshots.length > 0) {
        const primaryIdx = llm.buttonClassification.primaryButtonIndex;
        const secondaryIdx = llm.buttonClassification.secondaryButtonIndex;
        if (primaryIdx >= 0 && primaryIdx < buttonSnapshots.length) {
            const primaryBtn = buttonSnapshots[primaryIdx];
            if (!merged.components)
                merged.components = {};
            merged.components.buttonPrimary = {
                background: primaryBtn.background,
                textColor: primaryBtn.textColor,
                borderColor: primaryBtn.borderColor,
                borderRadius: primaryBtn.borderRadius || "0px",
                borderRadiusCorners: primaryBtn.borderRadiusCorners,
                shadow: primaryBtn.shadow,
            };
        }
        if (secondaryIdx >= 0 && secondaryIdx < buttonSnapshots.length) {
            const secondaryBtn = buttonSnapshots[secondaryIdx];
            const primaryBtn = buttonSnapshots[primaryIdx];
            if (!primaryBtn || secondaryBtn.background !== primaryBtn.background) {
                if (!merged.components)
                    merged.components = {};
                merged.components.buttonSecondary = {
                    background: secondaryBtn.background,
                    textColor: secondaryBtn.textColor,
                    borderColor: secondaryBtn.borderColor,
                    borderRadius: secondaryBtn.borderRadius || "0px",
                    borderRadiusCorners: secondaryBtn.borderRadiusCorners,
                    shadow: secondaryBtn.shadow,
                };
            }
        }
    }
    if (llm.colorRoles.confidence > 0.7) {
        merged.colors = {
            ...merged.colors,
            primary: llm.colorRoles.primaryColor || merged.colors?.primary,
            accent: llm.colorRoles.accentColor || merged.colors?.accent,
            background: llm.colorRoles.backgroundColor || merged.colors?.background,
            textPrimary: llm.colorRoles.textPrimary || merged.colors?.textPrimary,
        };
    }
    if (llm.personality) {
        merged.personality = llm.personality;
    }
    if (llm.designSystem) {
        merged.designSystem = llm.designSystem;
    }
    if (llm.cleanedFonts && llm.cleanedFonts.length > 0) {
        merged.fonts = llm.cleanedFonts;
        const cleanFontName = (font) => {
            const fontLower = font.toLowerCase();
            for (const cleanedFont of llm.cleanedFonts) {
                const cleanedLower = cleanedFont.family.toLowerCase();
                if (fontLower === cleanedLower)
                    return cleanedFont.family;
                if (fontLower.includes(cleanedLower))
                    return cleanedFont.family;
                const nextJsPattern = /^__(.+?)(?:_Fallback)?_[a-f0-9]{8}$/i;
                const match = font.match(nextJsPattern);
                if (match) {
                    const extractedName = match[1].toLowerCase();
                    if (extractedName === cleanedLower ||
                        cleanedLower.includes(extractedName)) {
                        return cleanedFont.family;
                    }
                }
            }
            return font;
        };
        if (merged.typography?.fontStacks &&
            Array.isArray(merged.typography.fontStacks)) {
            const stacks = merged.typography.fontStacks;
            const cleanStack = (stack) => {
                if (!stack)
                    return stack;
                const cleaned = stack.map(cleanFontName);
                const seen = new Set();
                return cleaned.filter((font) => {
                    if (seen.has(font.toLowerCase()))
                        return false;
                    seen.add(font.toLowerCase());
                    return true;
                });
            };
            merged.typography.fontStacks = {
                primary: cleanStack(stacks.primary),
                heading: cleanStack(stacks.heading),
                body: cleanStack(stacks.body),
                paragraph: cleanStack(stacks.paragraph),
            };
        }
        if (merged.typography?.fontFamilies) {
            const headingFont = llm.cleanedFonts.find((f) => f.role === "heading");
            const bodyFont = llm.cleanedFonts.find((f) => f.role === "body");
            const displayFont = llm.cleanedFonts.find((f) => f.role === "display");
            const primaryFont = bodyFont || llm.cleanedFonts[0];
            if (primaryFont) {
                merged.typography.fontFamilies.primary = primaryFont.family;
            }
            const headingToUse = headingFont || displayFont || primaryFont;
            if (headingToUse) {
                merged.typography.fontFamilies.heading = headingToUse.family;
            }
        }
    }
    merged.confidence = {
        buttons: llm.buttonClassification.confidence,
        colors: llm.colorRoles.confidence,
        overall: (llm.buttonClassification.confidence + llm.colorRoles.confidence) / 2,
    };
    return merged;
}
//# sourceMappingURL=merge.js.map