
use crate::models::*;

pub fn apply_rules(report: &mut SeoReport) {
    if report.title.is_none() {
        report.issues.push(SeoIssue {
            code: "SEO-META-001",
            severity: Severity::Critical,
            message: "Missing <title> tag".into(),
            hint: Some("Add a 50–60 character title".into()),
        });
    }

    if report.meta_description.is_none() {
        report.issues.push(SeoIssue {
            code: "SEO-META-002",
            severity: Severity::High,
            message: "Missing meta description".into(),
            hint: Some("Add a 140–160 character description".into()),
        });
    }

    let h1_count = report.headings.iter().filter(|h| h.level == 1).count();
    if h1_count == 0 {
        report.issues.push(SeoIssue {
            code: "SEO-H1-001",
            severity: Severity::Critical,
            message: "No H1 found".into(),
            hint: Some("Add exactly one H1".into()),
        });
    } else if h1_count > 1 {
        report.issues.push(SeoIssue {
            code: "SEO-H1-002",
            severity: Severity::Critical,
            message: "Multiple H1 tags found".into(),
            hint: Some("Keep exactly one H1".into()),
        });
    }

    if report.word_count < 300 {
        report.issues.push(SeoIssue {
            code: "SEO-CONTENT-001",
            severity: Severity::Medium,
            message: "Thin content".into(),
            hint: Some("Expand content to at least 300 words".into()),
        });
    }

    for img in &report.images {
        if img.alt.as_deref().unwrap_or("").is_empty() {
            report.issues.push(SeoIssue {
                code: "SEO-IMG-001",
                severity: Severity::Medium,
                message: format!("Image {} missing alt text", img.src),
                hint: Some("Add descriptive alt text".into()),
            });
        }
        if img.loading.is_none() {
            report.issues.push(SeoIssue {
                code: "SEO-IMG-002",
                severity: Severity::Low,
                message: format!("Image {} missing loading attribute", img.src),
                hint: Some("Use loading=lazy".into()),
            });
        }
    }

    for link in &report.links {
        if link.text.trim().is_empty() {
            report.issues.push(SeoIssue {
                code: "SEO-LINK-001",
                severity: Severity::Low,
                message: format!("Link {} has empty text", link.href),
                hint: Some("Use descriptive anchor text".into()),
            });
        }
    }
}
