/**
 * Branding types - adapted from Firecrawl for Workers.
 */

export interface BrandingProfile {
  colorScheme?: "light" | "dark";
  logo?: string | null;
  fonts?: Array<{
    family: string;
    count?: number;
    [key: string]: unknown;
  }>;
  colors?: {
    primary?: string;
    secondary?: string;
    accent?: string;
    background?: string;
    textPrimary?: string;
    textSecondary?: string;
    link?: string;
    success?: string;
    warning?: string;
    error?: string;
    [key: string]: string | undefined;
  };
  typography?: {
    fontFamilies?: {
      primary?: string;
      heading?: string;
      code?: string;
      [key: string]: string | undefined;
    };
    fontStacks?: {
      primary?: string[];
      heading?: string[];
      body?: string[];
      paragraph?: string[];
      [key: string]: string[] | undefined;
    };
    fontSizes?: {
      h1?: string;
      h2?: string;
      h3?: string;
      body?: string;
      small?: string;
      [key: string]: string | undefined;
    };
    [key: string]: unknown;
  };
  spacing?: {
    baseUnit?: number;
    borderRadius?: string;
    [key: string]: unknown;
  };
  components?: Record<string, unknown>;
  images?: {
    logo?: string | null;
    logoHref?: string | null;
    logoAlt?: string | null;
    favicon?: string | null;
    ogImage?: string | null;
    [key: string]: string | null | undefined;
  };
  [key: string]: unknown;
}

export interface ButtonSnapshot {
  index: number;
  text: string;
  html: string;
  classes: string;
  background: string;
  textColor: string;
  borderColor?: string | null;
  borderRadius?: string;
  borderRadiusCorners?: {
    topLeft?: string;
    topRight?: string;
    bottomRight?: string;
    bottomLeft?: string;
  };
  shadow?: string | null;
  originalBackgroundColor?: string;
  originalTextColor?: string;
  originalBorderColor?: string;
}

export interface InputSnapshot {
  type: string;
  placeholder: string;
  label: string;
  name: string;
  required: boolean;
  classes: string;
  background: string;
  textColor: string | null;
  borderColor?: string | null;
  borderRadius?: string;
  borderRadiusCorners?: Record<string, string>;
  shadow?: string | null;
}

export interface BrandingLLMInput {
  jsAnalysis: BrandingProfile;
  buttons: ButtonSnapshot[];
  logoCandidates?: Array<{
    src: string;
    alt: string;
    ariaLabel?: string;
    title?: string;
    isSvg: boolean;
    isVisible: boolean;
    location: "header" | "body" | "footer";
    position: { top: number; left: number; width: number; height: number };
    indicators: {
      inHeader: boolean;
      altMatch: boolean;
      srcMatch: boolean;
      classMatch: boolean;
      hrefMatch: boolean;
    };
    href?: string;
    source: string;
    logoSvgScore?: number;
  }>;
  brandName?: string;
  pageTitle?: string;
  pageUrl?: string;
  backgroundCandidates?: Array<{
    color: string;
    source: string;
    priority: number;
    area?: number;
  }>;
  screenshot?: string;
  url: string;
  headerHtmlChunk?: string;
  favicon?: string | null;
  ogImage?: string | null;
  heuristicLogoPick?: {
    selectedIndexInFilteredList: number;
    confidence: number;
    reasoning: string;
  };
  teamId?: string;
  teamFlags?: { debugBranding?: boolean } | null;
}

/**
 * Data structure returned by the branding script (inner `branding` property).
 */
export interface BrandingScriptReturn {
  cssData: {
    colors: string[];
    spacings: number[];
    radii: number[];
  };
  snapshots: Array<{
    tag: string;
    classes: string;
    text: string;
    rect: { w: number; h: number };
    colors: {
      text: string;
      background: string;
      border: string;
      borderWidth: number | null;
      [key: string]: unknown;
    };
    typography: {
      fontStack: string[];
      size: string | null;
      weight: number | null;
    };
    radius: number | null;
    borderRadius: {
      topLeft: number | null;
      topRight: number | null;
      bottomRight: number | null;
      bottomLeft: number | null;
    };
    shadow: string | null;
    isButton: boolean;
    isNavigation?: boolean;
    hasCTAIndicator?: boolean;
    isInput: boolean;
    inputMetadata?: {
      type: string;
      placeholder: string;
      value: string;
      required: boolean;
      disabled: boolean;
      name: string;
      id: string;
      label: string;
    } | null;
    isLink: boolean;
  }>;
  images: Array<{ type: string; src: string }>;
  logoCandidates?: Array<{
    src: string;
    alt: string;
    ariaLabel?: string;
    title?: string;
    isSvg: boolean;
    isVisible: boolean;
    location: "header" | "body" | "footer";
    position: { top: number; left: number; width: number; height: number };
    indicators: {
      inHeader: boolean;
      altMatch: boolean;
      srcMatch: boolean;
      classMatch: boolean;
      hrefMatch: boolean;
    };
    href?: string;
    source: string;
    logoSvgScore?: number;
  }>;
  brandName?: string;
  pageTitle?: string;
  pageUrl?: string;
  typography: {
    stacks: {
      body: string[];
      heading: string[];
      paragraph: string[];
    };
    sizes: {
      h1: string;
      h2: string;
      body: string;
    };
  };
  frameworkHints: string[];
  colorScheme: "light" | "dark";
  pageBackground?: string | null;
  backgroundCandidates?: Array<{
    color: string;
    source: string;
    priority: number;
    area?: number;
  }>;
}

export function calculateLogoArea(position?: {
  width?: number;
  height?: number;
}): number {
  if (!position) return 0;
  return (position.width || 0) * (position.height || 0);
}
