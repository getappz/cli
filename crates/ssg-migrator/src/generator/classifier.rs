//! Static safety classifier for React to Astro conversion.

use biome_js_syntax::{JsSyntaxKind, JsSyntaxNode};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticSafety {
    Safe,
    Unsafe,
}

#[derive(Debug, Clone)]
pub enum UnsafeReason {
    UnresolvedJsxIdentifier(String),
    FunctionScopedIdentifier(String),
    ReactHookUsage(String),
    #[allow(dead_code)]
    BrowserApiUsage(String),
}

pub struct StaticSafetyResult {
    pub safety: StaticSafety,
    pub reasons: Vec<UnsafeReason>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeclaredKind {
    Import,
    ModuleConst,
    FunctionScoped,
}

pub struct StaticSafetyClassifier {
    declared: HashMap<String, DeclaredKind>,
    jsx_used: std::collections::HashSet<String>,
    reasons: Vec<UnsafeReason>,
}

const CONTEXT_DEPENDENT_COMPONENTS: &[&str] = &[
    "Accordion", "AccordionItem", "AccordionTrigger", "AccordionContent",
    "Dialog", "DialogTrigger", "DialogContent",
    "DropdownMenu", "DropdownMenuTrigger", "DropdownMenuContent",
    "Popover", "PopoverTrigger", "PopoverContent",
    "Select", "SelectTrigger", "SelectContent",
    "Tabs", "TabsList", "TabsTrigger", "TabsContent",
    "Tooltip", "TooltipTrigger", "TooltipContent",
    "Toast", "ToastProvider",
];

impl StaticSafetyClassifier {
    pub fn classify(root: &JsSyntaxNode) -> StaticSafetyResult {
        let mut classifier = Self {
            declared: HashMap::new(),
            jsx_used: std::collections::HashSet::new(),
            reasons: Vec::new(),
        };
        classifier.collect_declarations(root);
        classifier.collect_jsx_usages(root);
        classifier.evaluate();
        StaticSafetyResult {
            safety: if classifier.reasons.is_empty() {
                StaticSafety::Safe
            } else {
                StaticSafety::Unsafe
            },
            reasons: classifier.reasons,
        }
    }

    fn collect_declarations(&mut self, root: &JsSyntaxNode) {
        for node in root.descendants() {
            match node.kind() {
                JsSyntaxKind::JS_IMPORT_DEFAULT_CLAUSE
                | JsSyntaxKind::JS_IMPORT_NAMESPACE_CLAUSE => {
                    if let Some(id) = node.first_token() {
                        let name = id.text_trimmed();
                        if !name.is_empty() {
                            self.declared.insert(name.to_string(), DeclaredKind::Import);
                        }
                    }
                }
                kind if kind
                    .to_string()
                    .map(|s| s.contains("IMPORT") && s.contains("SPECIFIER"))
                    .unwrap_or(false) =>
                {
                    for child in node.descendants() {
                        if child.kind() == JsSyntaxKind::JS_REFERENCE_IDENTIFIER {
                            let name = child.text_trimmed();
                            if !name.is_empty() {
                                self.declared.insert(name.to_string(), DeclaredKind::Import);
                            }
                        }
                    }
                }
                JsSyntaxKind::JS_VARIABLE_DECLARATOR => {
                    if let Some(binding) = node
                        .children()
                        .find(|c| c.kind() == JsSyntaxKind::JS_IDENTIFIER_BINDING)
                    {
                        let name = binding.text_trimmed();
                        if !name.is_empty() {
                            let kind = if is_inside_function(&node) {
                                DeclaredKind::FunctionScoped
                            } else {
                                DeclaredKind::ModuleConst
                            };
                            self.declared.insert(name.to_string(), kind);
                        }
                    }
                }
                JsSyntaxKind::JS_FUNCTION_DECLARATION => {
                    if let Some(id) = node
                        .children()
                        .find(|c| c.kind() == JsSyntaxKind::JS_IDENTIFIER_BINDING)
                    {
                        let name = id.text_trimmed();
                        if !name.is_empty() {
                            self.declared
                                .insert(name.to_string(), DeclaredKind::ModuleConst);
                        }
                    }
                }
                JsSyntaxKind::TS_TYPE_ALIAS_DECLARATION
                | JsSyntaxKind::TS_INTERFACE_DECLARATION => {
                    if let Some(id) = node
                        .children()
                        .find(|c| c.kind() == JsSyntaxKind::TS_IDENTIFIER_BINDING)
                    {
                        let name = id.text_trimmed();
                        if !name.is_empty() {
                            self.declared
                                .insert(name.to_string(), DeclaredKind::ModuleConst);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn collect_jsx_usages(&mut self, root: &JsSyntaxNode) {
        for node in root.descendants() {
            if node.kind() == JsSyntaxKind::JS_REFERENCE_IDENTIFIER && is_inside_jsx(&node) {
                let name = node.text_trimmed();
                if name == "Astro" || name == "props" {
                    continue;
                }
                if !name.is_empty() {
                    self.jsx_used.insert(name.to_string());
                }
            }
        }
    }

    fn evaluate(&mut self) {
        for &component in CONTEXT_DEPENDENT_COMPONENTS {
            if self.jsx_used.contains(component) {
                self.reasons.push(UnsafeReason::ReactHookUsage(format!(
                    "Component '{}' requires React context",
                    component
                )));
            }
        }

        for ident in &self.jsx_used {
            match self.declared.get(ident) {
                Some(DeclaredKind::Import) | Some(DeclaredKind::ModuleConst) => {}
                Some(DeclaredKind::FunctionScoped) => {
                    self.reasons
                        .push(UnsafeReason::FunctionScopedIdentifier(ident.clone()));
                }
                None => {
                    self.reasons
                        .push(UnsafeReason::UnresolvedJsxIdentifier(ident.clone()));
                }
            }
        }
    }
}

fn is_inside_jsx(node: &JsSyntaxNode) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if let Some(kind_str) = p.kind().to_string() {
            if kind_str.starts_with("JSX_") {
                return true;
            }
        }
        parent = p.parent();
    }
    false
}

fn is_inside_function(node: &JsSyntaxNode) -> bool {
    let mut parent = node.parent();
    while let Some(p) = parent {
        match p.kind() {
            JsSyntaxKind::JS_FUNCTION_DECLARATION
            | JsSyntaxKind::JS_FUNCTION_EXPRESSION
            | JsSyntaxKind::JS_ARROW_FUNCTION_EXPRESSION => return true,
            _ => parent = p.parent(),
        }
    }
    false
}
