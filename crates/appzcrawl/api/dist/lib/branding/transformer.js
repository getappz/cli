/**
 * Transform raw branding script output into BrandingProfile.
 * Adapted from Firecrawl for Workers (heuristic + optional LLM).
 */
import { extractHeaderHtmlChunk } from "./extractHeaderHtmlChunk";
import { enhanceBrandingWithLLM } from "./llm";
import { getTopCandidatesForLLM, selectLogoWithConfidence, } from "./logo-selector";
import { mergeBrandingResults } from "./merge";
import { processRawBranding } from "./processor";
/**
 * Transform raw branding script output into a BrandingProfile.
 * Uses heuristics for logo/button selection; LLM stub returns heuristic-only enhancement.
 */
export async function brandingTransformer(input) {
    const jsBranding = processRawBranding(input.rawBranding);
    if (!jsBranding) {
        return {};
    }
    let brandingProfile = jsBranding;
    const buttonSnapshots = jsBranding.__button_snapshots ?? [];
    const logoCandidates = input.rawBranding.logoCandidates ?? [];
    const brandName = input.rawBranding.brandName;
    const backgroundCandidates = input.rawBranding.backgroundCandidates ?? [];
    let heuristicResult = null;
    try {
        heuristicResult =
            logoCandidates.length > 0
                ? selectLogoWithConfidence(logoCandidates, brandName)
                : null;
        const { filteredCandidates, indexMap } = logoCandidates.length > 0
            ? getTopCandidatesForLLM(logoCandidates, 20)
            : { filteredCandidates: [], indexMap: new Map() };
        let heuristicLogoPick;
        if (heuristicResult && filteredCandidates.length > 0) {
            let heuristicFilteredIndex = -1;
            for (const [filteredIdx, originalIdx] of indexMap) {
                if (originalIdx === heuristicResult.selectedIndex) {
                    heuristicFilteredIndex = filteredIdx;
                    break;
                }
            }
            if (heuristicFilteredIndex >= 0) {
                heuristicLogoPick = {
                    selectedIndexInFilteredList: heuristicFilteredIndex,
                    confidence: heuristicResult.confidence,
                    reasoning: heuristicResult.reasoning,
                };
            }
        }
        let limitedButtons = buttonSnapshots;
        const buttonIndexMap = new Map();
        if (buttonSnapshots.length > 12) {
            const scored = buttonSnapshots
                .map((btn, idx) => {
                let score = 0;
                const text = (btn.text || "").toLowerCase();
                const bgColor = btn.background || "";
                const primaryCtaKeywords = [
                    "get started",
                    "sign up",
                    "sign in",
                    "login",
                    "register",
                    "read",
                    "learn",
                    "download",
                    "buy",
                    "shop",
                ];
                if (primaryCtaKeywords.some((kw) => text.includes(kw)))
                    score += 100;
                const secondaryCtaKeywords = ["try", "start", "view", "explore"];
                if (secondaryCtaKeywords.some((kw) => text.includes(kw)))
                    score += 50;
                if (bgColor &&
                    !bgColor.match(/transparent|white|#fff|#ffffff|gray|grey|#f[0-9a-f]{5}/i)) {
                    score += 30;
                }
                return { btn, originalIdx: idx, score };
            })
                .sort((a, b) => b.score - a.score)
                .slice(0, 12);
            limitedButtons = scored.map((item) => item.btn);
            scored.forEach((item, llmIdx) => {
                buttonIndexMap.set(llmIdx, item.originalIdx);
            });
        }
        else {
            for (let i = 0; i < buttonSnapshots.length; i++) {
                buttonIndexMap.set(i, i);
            }
        }
        const headerHtmlChunk = logoCandidates.length === 0 &&
            input.html &&
            typeof input.html === "string"
            ? extractHeaderHtmlChunk(input.html)
            : undefined;
        const llmEnhancement = await enhanceBrandingWithLLM({
            jsAnalysis: jsBranding,
            buttons: limitedButtons,
            logoCandidates: filteredCandidates.length > 0 ? filteredCandidates : undefined,
            brandName,
            pageTitle: input.rawBranding.pageTitle,
            pageUrl: input.rawBranding.pageUrl,
            backgroundCandidates: backgroundCandidates.length > 0 ? backgroundCandidates : undefined,
            url: input.url,
            headerHtmlChunk: headerHtmlChunk ?? undefined,
            favicon: brandingProfile.images?.favicon ?? undefined,
            ogImage: brandingProfile.images?.ogImage ?? undefined,
            heuristicLogoPick,
            teamFlags: input.debugBranding ? { debugBranding: true } : undefined,
        }, input.aiBinding);
        if (llmEnhancement.logoSelection && logoCandidates.length > 0) {
            const llmFilteredIndex = llmEnhancement.logoSelection.selectedLogoIndex;
            const llmOriginalIndex = indexMap.get(llmFilteredIndex);
            if (llmOriginalIndex !== undefined) {
                llmEnhancement.logoSelection.selectedLogoIndex = llmOriginalIndex;
            }
            else if (llmFilteredIndex >= 0 && heuristicResult) {
                llmEnhancement.logoSelection = {
                    selectedLogoIndex: heuristicResult.selectedIndex,
                    selectedLogoReasoning: `Heuristic fallback (LLM returned invalid index): ${heuristicResult.reasoning}`,
                    confidence: Math.max(heuristicResult.confidence - 0.1, 0.3),
                };
            }
        }
        if (heuristicResult &&
            logoCandidates.length > 0 &&
            !llmEnhancement.logoSelection) {
            llmEnhancement.logoSelection = {
                selectedLogoIndex: heuristicResult.selectedIndex,
                selectedLogoReasoning: `Heuristic fallback: ${heuristicResult.reasoning}`,
                confidence: Math.max(heuristicResult.confidence - 0.1, 0.3),
            };
        }
        brandingProfile = mergeBrandingResults(jsBranding, llmEnhancement, buttonSnapshots, logoCandidates.length > 0
            ? logoCandidates
            : undefined);
    }
    catch {
        brandingProfile = jsBranding;
    }
    if (!input.debugBranding) {
        delete brandingProfile.__button_snapshots;
        delete brandingProfile.__input_snapshots;
        delete brandingProfile.__logo_candidates;
    }
    return brandingProfile;
}
//# sourceMappingURL=transformer.js.map