/**
 * Process raw branding script output into BrandingProfile.
 * Uses culori for color parsing ( Workers-compatible).
 */
import type { BrandingProfile, BrandingScriptReturn } from "./types";
export declare function hexify(rgba: string, background?: string | null): string | null;
export declare function processRawBranding(raw: BrandingScriptReturn): BrandingProfile;
//# sourceMappingURL=processor.d.ts.map