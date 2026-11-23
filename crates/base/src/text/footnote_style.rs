use gpui::{Rems, SharedString, rems};

use super::node::BlockNode;

/// Opt-in typography and hanging geometry for native Markdown footnotes.
///
/// References keep their identifiers and entries stay in source order. No
/// navigation or task behavior is installed. Scales are relative to inherited
/// text size; `reference_raise` uses that size's em. Lengths use window rems.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FootnoteStyle {
    reference_scale: f32,
    reference_raise: f32,
    entry_scale: f32,
    indent: Rems,
    entry_gap: Option<Rems>,
}

impl Default for FootnoteStyle {
    fn default() -> Self {
        Self {
            reference_scale: 1.,
            reference_raise: 0.,
            entry_scale: 1.,
            indent: rems(0.),
            entry_gap: None,
        }
    }
}

impl FootnoteStyle {
    /// Inherited text sizes with a naturally fitted hanging label lane.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets reference size relative to inherited text (default 1).
    /// Nonpositive or non-finite scales leave the current value unchanged.
    pub fn with_reference_scale(mut self, scale: f32) -> Self {
        if scale.is_finite() && scale > 0. {
            self.reference_scale = scale;
        }
        self
    }

    /// Raises references in inherited text ems (default 0).
    /// Negative values clamp to zero; non-finite values are ignored.
    pub fn with_reference_raise(mut self, raise: f32) -> Self {
        if raise.is_finite() {
            self.reference_raise = raise.max(0.);
        }
        self
    }

    /// Sets entry size relative to its container's text (default 1).
    /// Nonpositive or non-finite scales are ignored.
    pub fn with_entry_scale(mut self, scale: f32) -> Self {
        if scale.is_finite() && scale > 0. {
            self.entry_scale = scale;
        }
        self
    }

    /// Sets the minimum shared label lane (default 0 rem, fitted to labels).
    /// Negative lengths clamp to zero; non-finite lengths are ignored.
    pub fn with_indent(mut self, indent: Rems) -> Self {
        if indent.0.is_finite() {
            self.indent = rems(indent.0.max(0.));
        }
        self
    }

    /// Sets spacing between consecutive entries, not before following prose.
    /// List entries retain their list's spacing.
    /// By default it inherits the paragraph gap. Negative lengths clamp to
    /// zero; non-finite lengths are ignored.
    pub fn with_entry_gap(mut self, gap: Rems) -> Self {
        if gap.0.is_finite() {
            self.entry_gap = Some(rems(gap.0.max(0.)));
        }
        self
    }

    /// Reference font size relative to inherited text.
    pub fn reference_scale(&self) -> f32 {
        self.reference_scale
    }

    /// Reference baseline raise in inherited text ems.
    pub fn reference_raise(&self) -> f32 {
        self.reference_raise
    }

    /// Entry font size relative to its container.
    pub fn entry_scale(&self) -> f32 {
        self.entry_scale
    }

    /// Minimum hanging lane, widened to fit the document's labels.
    pub fn indent(&self) -> Rems {
        self.indent
    }

    /// Inter-entry spacing, or `None` to inherit the paragraph gap.
    pub fn entry_gap(&self) -> Option<Rems> {
        self.entry_gap
    }
}

pub(super) fn prefixes(blocks: &[BlockNode]) -> Vec<SharedString> {
    let mut prefixes = Vec::new();
    collect_prefixes(blocks, &mut prefixes);
    prefixes
}

fn collect_prefixes(blocks: &[BlockNode], prefixes: &mut Vec<SharedString>) {
    for block in blocks {
        match block {
            BlockNode::Paragraph(paragraph) => {
                if let Some(len) = paragraph.footnote_prefix {
                    let text = paragraph.text();
                    if let Some(prefix) = text.get(..len) {
                        prefixes.push(prefix.to_string().into());
                    }
                }
            }
            BlockNode::Root { children, .. }
            | BlockNode::Blockquote { children, .. }
            | BlockNode::Surface { children, .. }
            | BlockNode::List { children, .. }
            | BlockNode::ListItem { children, .. } => collect_prefixes(children, prefixes),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextViewStyle;

    #[test]
    fn footnote_style_is_opt_in_and_all_values_affect_equality() {
        let neutral = TextViewStyle::default();
        assert!(neutral.footnote_style().is_none());
        let style = FootnoteStyle::new();
        assert_eq!(style.reference_scale(), 1.);
        assert_eq!(style.reference_raise(), 0.);
        assert_eq!(style.entry_scale(), 1.);
        assert_eq!(style.indent(), rems(0.));
        assert_eq!(style.entry_gap(), None);
        let enabled = neutral.clone().with_footnote_style(style);
        assert!(neutral != enabled);
        assert_eq!(enabled.footnote_style(), Some(&style));
        for changed in [
            style.with_reference_scale(0.7),
            style.with_reference_raise(0.3),
            style.with_entry_scale(0.8),
            style.with_indent(rems(2.)),
            style.with_entry_gap(rems(0.2)),
        ] {
            assert!(enabled != neutral.clone().with_footnote_style(changed));
        }
    }

    #[test]
    fn footnote_prefix_cache_is_derived_recursive_and_shared_when_cloned() {
        use crate::text::{format::markdown, node::NodeContext};
        use std::sync::Arc;
        let document = markdown::parse(
            "[^a]: Root\n\n> [^long]: Nested",
            &mut NodeContext::default(),
        )
        .unwrap();
        let uncached = document.clone();
        let labels = document.footnote_prefixes();
        assert_eq!(
            labels.iter().map(|s| s.as_ref()).collect::<Vec<_>>(),
            ["[a]: ", "[long]: "]
        );
        assert_eq!(document, uncached, "cache initialization is not content");
        let cloned = document.clone();
        assert!(Arc::ptr_eq(&labels, &cloned.footnote_prefixes()));
        assert!(Arc::ptr_eq(&labels, &document.footnote_prefixes()));
    }

    #[test]
    fn footnote_lookahead_skips_definitions_but_not_visible_prose_headings_or_rules() {
        use crate::text::{document::next_is_footnote, format::markdown, node::NodeContext};
        for following in ["Prose", "# Heading", "---", "[^b]: Next"] {
            let source = format!("[^a]: Root\n\n[link]: https://example.com\n\n{following}");
            let document = markdown::parse(&source, &mut NodeContext::default()).unwrap();
            assert_eq!(
                next_is_footnote(&document.blocks, 0),
                following.starts_with("[^b]")
            );
        }
    }

    #[test]
    fn footnote_style_rejects_invalid_scales_and_sanitizes_lengths() {
        let style = FootnoteStyle::new();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                style
                    .with_reference_scale(value)
                    .with_reference_raise(value)
                    .with_entry_scale(value)
                    .with_indent(rems(value))
                    .with_entry_gap(rems(value)),
                style
            );
        }
        for value in [0., -1.] {
            assert_eq!(
                style.with_reference_scale(value).with_entry_scale(value),
                style
            );
        }
        let clamped = style
            .with_reference_raise(-1.)
            .with_indent(rems(-1.))
            .with_entry_gap(rems(-1.));
        assert_eq!(clamped.reference_raise(), 0.);
        assert_eq!(clamped.indent(), rems(0.));
        assert_eq!(clamped.entry_gap(), Some(rems(0.)));
    }
}
