/**
 * Heuristic logo selection - no LLM dependency.
 */
import { logger } from "../logger";
import { calculateLogoArea } from "./types";
const CONFIDENCE_THRESHOLDS = {
    STRONG_SCORE: 60,
    GOOD_SCORE: 45,
    MODERATE_SCORE: 30,
    STRONG_SEPARATION: 20,
    GOOD_SEPARATION: 15,
    STRONG_CONFIDENCE: 0.9,
    GOOD_CONFIDENCE: 0.75,
    MODERATE_CONFIDENCE: 0.6,
    WEAK_CONFIDENCE: 0.4,
};
function extractFilename(src) {
    if (!src)
        return null;
    const withoutQuery = src.split("?")[0];
    const parts = withoutQuery.split("/");
    const filename = parts.pop();
    return filename || null;
}
function scoreHrefAndHeaderIndicators(candidate) {
    let score = 0;
    const reasons = [];
    if (candidate.indicators.hrefMatch && candidate.indicators.inHeader) {
        score += 50;
        reasons.push("header logo linking to homepage");
    }
    else if (candidate.indicators.hrefMatch) {
        score += 35;
        reasons.push("links to homepage");
    }
    else if (candidate.indicators.inHeader) {
        score += 25;
        reasons.push("in header");
    }
    if (!candidate.href || candidate.href.trim() === "") {
        score -= 15;
        reasons.push("no link (brand logos usually link to homepage, penalty)");
    }
    return { score, reasons };
}
function detectLogoVariants(candidates) {
    const groups = new Map();
    const processed = new Set();
    for (const [index, candidate] of candidates.entries()) {
        if (processed.has(index))
            continue;
        const similarIndices = [index];
        processed.add(index);
        for (const [otherIndex, other] of candidates.entries()) {
            if (index === otherIndex || processed.has(otherIndex))
                continue;
            const candidateFilename = extractFilename(candidate.src);
            const otherFilename = extractFilename(other.src);
            const isSimilar = (candidate.alt &&
                other.alt &&
                candidate.alt.toLowerCase().replace(/\s+/g, "") ===
                    other.alt.toLowerCase().replace(/\s+/g, "")) ||
                candidate.src === other.src ||
                (candidateFilename &&
                    otherFilename &&
                    candidate.src.includes(otherFilename) &&
                    candidateFilename === otherFilename) ||
                (Math.abs(candidate.position.top - other.position.top) < 20 &&
                    Math.abs(candidate.position.left - other.position.left) < 50 &&
                    Math.abs(candidate.position.width - other.position.width) < 30);
            if (isSimilar) {
                similarIndices.push(otherIndex);
                processed.add(otherIndex);
            }
        }
        if (similarIndices.length > 0) {
            groups.set(index, similarIndices);
        }
    }
    return groups;
}
function pickBestVariant(candidates, variantIndices) {
    return variantIndices.reduce((best, current) => {
        const bestCandidate = candidates[best];
        const currentCandidate = candidates[current];
        if (currentCandidate.isVisible && !bestCandidate.isVisible)
            return current;
        if (!currentCandidate.isVisible && bestCandidate.isVisible)
            return best;
        if (currentCandidate.indicators.inHeader &&
            !bestCandidate.indicators.inHeader)
            return current;
        if (!currentCandidate.indicators.inHeader &&
            bestCandidate.indicators.inHeader)
            return best;
        if (currentCandidate.position.top < bestCandidate.position.top)
            return current;
        if (currentCandidate.position.top > bestCandidate.position.top)
            return best;
        if (currentCandidate.indicators.hrefMatch &&
            !bestCandidate.indicators.hrefMatch)
            return current;
        return best;
    });
}
function detectRepeatedLogos(candidates) {
    const repeated = new Set();
    const srcGroups = new Map();
    for (const [index, candidate] of candidates.entries()) {
        const srcKey = extractFilename(candidate.src)?.toLowerCase() || candidate.src;
        if (!srcGroups.has(srcKey)) {
            srcGroups.set(srcKey, []);
        }
        const group = srcGroups.get(srcKey);
        if (group) {
            group.push(index);
        }
    }
    for (const indices of srcGroups.values()) {
        if (indices.length > 1) {
            const locations = new Set(indices.map((i) => candidates[i].location));
            if (locations.size > 1) {
                for (const i of indices) {
                    repeated.add(i);
                }
            }
        }
    }
    return repeated;
}
export function selectLogoWithConfidence(candidates, brandName) {
    if (candidates.length === 0) {
        return {
            selectedIndex: -1,
            confidence: 0,
            method: "fallback",
            reasoning: "No logo candidates provided",
        };
    }
    const variantGroups = detectLogoVariants(candidates);
    const repeatedLogos = detectRepeatedLogos(candidates);
    logger.debug("Logo variant analysis", {
        totalCandidates: candidates.length,
        variantGroupsCount: variantGroups.size,
        repeatedLogosCount: repeatedLogos.size,
    });
    const indicesToScore = new Set();
    const variantBonuses = new Map();
    if (variantGroups.size > 0) {
        for (const variants of variantGroups.values()) {
            const bestIndex = pickBestVariant(candidates, variants);
            indicesToScore.add(bestIndex);
            if (variants.some((i) => repeatedLogos.has(i))) {
                variantBonuses.set(bestIndex, 15);
            }
            if (variants.length > 1) {
                variantBonuses.set(bestIndex, (variantBonuses.get(bestIndex) || 0) + 8);
            }
        }
    }
    else {
        for (let i = 0; i < candidates.length; i++) {
            indicesToScore.add(i);
        }
    }
    const scored = candidates.map((candidate, index) => {
        if (!indicesToScore.has(index)) {
            return {
                index,
                score: -999,
                candidate,
                reasons: "skipped (duplicate variant)",
            };
        }
        let score = 0;
        const reasons = [];
        const variantBonus = variantBonuses.get(index) || 0;
        if (variantBonus > 0) {
            score += variantBonus;
            reasons.push(`variant bonus (+${variantBonus})`);
        }
        const hrefHeaderScore = scoreHrefAndHeaderIndicators(candidate);
        score += hrefHeaderScore.score;
        reasons.push(...hrefHeaderScore.reasons);
        if (candidate.location === "header") {
            score += 20;
            reasons.push("header location");
        }
        if (candidate.isVisible) {
            score += 15;
            reasons.push("visible");
        }
        else {
            score -= 25;
            reasons.push("invisible (hidden/minimized variant, heavy penalty)");
        }
        if (candidate.position.top < 100 && candidate.position.left < 300) {
            score += 10;
            reasons.push("top-left position");
        }
        const isHighest = candidates.every((other, otherIndex) => otherIndex === index || candidate.position.top <= other.position.top);
        if (isHighest && candidate.position.top < 200) {
            score += 12;
            reasons.push("highest logo on page");
        }
        if (candidate.indicators.altMatch) {
            score += 8;
            reasons.push("alt matches logo/brand");
        }
        if (candidate.indicators.srcMatch) {
            score += 5;
            reasons.push("src contains logo");
        }
        if (candidate.indicators.classMatch) {
            score += 5;
            reasons.push("class contains logo");
        }
        if (brandName) {
            const altLower = candidate.alt.toLowerCase().trim();
            const brandLower = brandName.toLowerCase().trim();
            if (altLower === brandLower) {
                score += 20;
                reasons.push(`alt exactly matches brand name "${brandName}"`);
            }
            else if (altLower &&
                (altLower.includes(brandLower) || brandLower.includes(altLower))) {
                score += 12;
                reasons.push(`alt contains brand name "${brandName}"`);
            }
            if (candidate.src.toLowerCase().includes(brandLower)) {
                score += 6;
                reasons.push(`src contains brand name "${brandName}"`);
            }
        }
        const area = calculateLogoArea(candidate.position);
        const width = candidate.position.width;
        const height = candidate.position.height;
        if (area > 1000 && area < 50000) {
            score += 5;
            reasons.push("reasonable size");
        }
        else if (area < 500) {
            score -= 8;
            reasons.push("too small (likely icon, penalty)");
        }
        else if (area >= 50000 && area <= 200000) {
            score -= 10;
            reasons.push("too large (likely banner/og:image, penalty)");
        }
        else if (area > 200000) {
            score -= 20;
            reasons.push("extremely large (likely og:image, heavy penalty)");
        }
        const isSquare = Math.abs(width - height) < 5;
        if (isSquare && (width < 40 || height < 40)) {
            score -= 12;
            reasons.push("small square icon (likely UI icon, heavy penalty)");
        }
        if (candidate.isSvg) {
            score += 3;
            reasons.push("SVG format");
        }
        if (candidate.location === "footer") {
            score -= 15;
            reasons.push("footer location (penalty)");
        }
        if (candidate.location === "body" && !candidate.indicators.inHeader) {
            score -= 10;
            reasons.push("body location without header (penalty)");
        }
        return {
            index,
            score,
            candidate,
            reasons: reasons.join(", "),
        };
    });
    const validScored = scored.filter((s) => s.score > -900);
    validScored.sort((a, b) => b.score - a.score);
    if (validScored.length === 0) {
        return {
            selectedIndex: -1,
            confidence: 0,
            method: "fallback",
            reasoning: "All candidates were filtered out as duplicate variants",
        };
    }
    const top = validScored[0];
    const secondBest = validScored[1];
    const scoreSeparation = secondBest ? top.score - secondBest.score : top.score;
    if (top.score >= CONFIDENCE_THRESHOLDS.STRONG_SCORE &&
        scoreSeparation >= CONFIDENCE_THRESHOLDS.STRONG_SEPARATION) {
        return {
            selectedIndex: top.index,
            confidence: CONFIDENCE_THRESHOLDS.STRONG_CONFIDENCE,
            method: "heuristic",
            reasoning: `Strong indicators: ${top.reasons}. Score: ${top.score} (clear winner by ${scoreSeparation} points)`,
        };
    }
    if (top.score >= CONFIDENCE_THRESHOLDS.GOOD_SCORE &&
        scoreSeparation >= CONFIDENCE_THRESHOLDS.GOOD_SEPARATION) {
        return {
            selectedIndex: top.index,
            confidence: CONFIDENCE_THRESHOLDS.GOOD_CONFIDENCE,
            method: "heuristic",
            reasoning: `Good indicators: ${top.reasons}. Score: ${top.score} (ahead by ${scoreSeparation} points)`,
        };
    }
    if (top.score >= CONFIDENCE_THRESHOLDS.MODERATE_SCORE) {
        return {
            selectedIndex: top.index,
            confidence: CONFIDENCE_THRESHOLDS.MODERATE_CONFIDENCE,
            method: "heuristic",
            reasoning: `Moderate indicators: ${top.reasons}. Score: ${top.score}. May benefit from LLM validation.`,
        };
    }
    return {
        selectedIndex: top.index,
        confidence: CONFIDENCE_THRESHOLDS.WEAK_CONFIDENCE,
        method: "heuristic",
        reasoning: `Weak indicators: ${top.reasons}. Score: ${top.score}. LLM validation recommended (close scores: top=${top.score}, second=${secondBest?.score || 0})`,
    };
}
export function getTopCandidatesForLLM(candidates, maxCandidates = 10) {
    if (candidates.length <= maxCandidates) {
        const indexMap = new Map();
        for (let i = 0; i < candidates.length; i++) {
            indexMap.set(i, i);
        }
        return { filteredCandidates: candidates, indexMap };
    }
    const scored = candidates.map((candidate, originalIndex) => {
        let score = 0;
        const hrefHeaderScore = scoreHrefAndHeaderIndicators(candidate);
        score += hrefHeaderScore.score;
        if (candidate.location === "header")
            score += 20;
        if (candidate.isVisible)
            score += 15;
        if (candidate.indicators.classMatch)
            score += 10;
        if (candidate.indicators.srcMatch)
            score += 10;
        if (candidate.indicators.altMatch)
            score += 5;
        if (candidate.source === "document.images")
            score += 15;
        return { originalIndex, score, candidate };
    });
    scored.sort((a, b) => b.score - a.score);
    const topScored = scored.slice(0, maxCandidates);
    const indexMap = new Map();
    for (let i = 0; i < topScored.length; i++) {
        indexMap.set(i, topScored[i].originalIndex);
    }
    const filteredCandidates = topScored.map((s) => s.candidate);
    return { filteredCandidates, indexMap };
}
//# sourceMappingURL=logo-selector.js.map