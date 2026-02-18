/**
 * Merge JS analysis with LLM enhancement (or heuristic-only) into final BrandingProfile.
 */
import type { LogoCandidate } from "./logo-selector";
import type { BrandingEnhancement } from "./schema";
import type { BrandingProfile, ButtonSnapshot } from "./types";
export declare function mergeBrandingResults(js: BrandingProfile, llm: BrandingEnhancement, buttonSnapshots: ButtonSnapshot[], logoCandidates?: LogoCandidate[]): BrandingProfile;
//# sourceMappingURL=merge.d.ts.map