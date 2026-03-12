//! Rule Capability Matrix
//!
//! Defines the capabilities that SEO rules can have. Rules must declare their
//! capabilities explicitly to prevent impossible rules from running in wrong contexts.
//!
//! Capabilities:
//! - STREAMING_HTML: Can execute during streaming HTML parse
//! - FULL_DOCUMENT: Needs complete document in memory
//! - LIVE_HTTP: Requires HTTP requests
//! - AI_ASSISTED: Can use AI for suggestions (advisory only)

use bitflags::bitflags;

bitflags! {
    /// Capabilities that a rule can have.
    ///
    /// Rules must declare their capabilities explicitly. This enables:
    /// - Fast/slow passes (Phase 1: streaming, Phase 2: document, Phase 3: HTTP)
    /// - Feature gating (`--no-http`, `--no-ai`)
    /// - Deterministic CI runs
    /// - Performance optimization
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct RuleCapabilities: u32 {
        /// Can execute during streaming HTML parse (fastest, no DOM needed)
        const STREAMING_HTML = 0b0001;
        
        /// Needs complete document in memory (slower, requires full parse)
        const FULL_DOCUMENT  = 0b0010;
        
        /// Requires HTTP requests (slowest, network-dependent)
        const LIVE_HTTP      = 0b0100;
        
        /// Can use AI for suggestions (advisory only, never affects rule execution)
        const AI_ASSISTED    = 0b1000;
    }
}

impl RuleCapabilities {
    /// Check if rule can run in streaming mode
    pub fn can_stream(&self) -> bool {
        self.contains(RuleCapabilities::STREAMING_HTML)
    }
    
    /// Check if rule needs full document
    pub fn needs_document(&self) -> bool {
        self.contains(RuleCapabilities::FULL_DOCUMENT)
    }
    
    /// Check if rule requires HTTP
    pub fn needs_http(&self) -> bool {
        self.contains(RuleCapabilities::LIVE_HTTP)
    }
    
    /// Check if rule can use AI (advisory only)
    pub fn can_use_ai(&self) -> bool {
        self.contains(RuleCapabilities::AI_ASSISTED)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities() {
        let streaming = RuleCapabilities::STREAMING_HTML;
        assert!(streaming.can_stream());
        assert!(!streaming.needs_document());
        assert!(!streaming.needs_http());
        
        let document = RuleCapabilities::FULL_DOCUMENT;
        assert!(!document.can_stream());
        assert!(document.needs_document());
        
        let http = RuleCapabilities::LIVE_HTTP;
        assert!(http.needs_http());
        
        let combined = RuleCapabilities::STREAMING_HTML | RuleCapabilities::AI_ASSISTED;
        assert!(combined.can_stream());
        assert!(combined.can_use_ai());
    }
}

