use gpui::{Pixels, Rems, rems};

/// Opt-in geometry for native list markers, task checkboxes and continuations.
///
/// Lengths use the window's rem size. A list shares one marker lane, widened
/// to fit its longest marker or checkbox. These geometry values take precedence
/// over `TextViewStyle::list_style`; that refinement still styles marker text.
/// Installing this on a [`super::TextViewStyle`] does not enable task editing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ListLayout {
    indent: Rems,
    marker_gap: Rems,
    item_gap: Rems,
    loose_item_gap: Rems,
    paragraph_gap: Rems,
}

impl Default for ListLayout {
    fn default() -> Self {
        Self {
            indent: rems(1.4),
            marker_gap: rems(0.3),
            item_gap: rems(0.25),
            loose_item_gap: rems(0.75),
            paragraph_gap: rems(0.75),
        }
    }
}

impl ListLayout {
    /// The default geometry. Setters clamp negative lengths to zero and
    /// leave their current value unchanged for NaN or infinity.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the minimum lane from the list edge to its body spine (1.4 rem).
    pub fn with_indent(mut self, indent: Rems) -> Self {
        self.indent = length(indent, self.indent);
        self
    }

    /// Sets the gap between a marker and the body spine (0.3 rem).
    pub fn with_marker_gap(mut self, gap: Rems) -> Self {
        self.marker_gap = length(gap, self.marker_gap);
        self
    }

    /// Sets the gap between tight items and before a nested list (0.25 rem).
    pub fn with_item_gap(mut self, gap: Rems) -> Self {
        self.item_gap = length(gap, self.item_gap);
        self
    }

    /// Sets the sibling gap in a list containing loose items (0.75 rem).
    pub fn with_loose_item_gap(mut self, gap: Rems) -> Self {
        self.loose_item_gap = length(gap, self.loose_item_gap);
        self
    }

    /// Sets the gap before other continuation blocks in an item (0.75 rem).
    pub fn with_paragraph_gap(mut self, gap: Rems) -> Self {
        self.paragraph_gap = length(gap, self.paragraph_gap);
        self
    }

    /// The minimum shared marker lane, including its gap.
    pub fn indent(&self) -> Rems {
        self.indent
    }

    /// The space between a marker and its body.
    pub fn marker_gap(&self) -> Rems {
        self.marker_gap
    }

    /// The tight sibling gap and the gap before nested lists.
    pub fn item_gap(&self) -> Rems {
        self.item_gap
    }

    /// The sibling gap for loose lists.
    pub fn loose_item_gap(&self) -> Rems {
        self.loose_item_gap
    }

    /// The gap before continuation paragraphs and other non-list blocks.
    pub fn paragraph_gap(&self) -> Rems {
        self.paragraph_gap
    }
}

fn length(value: Rems, current: Rems) -> Rems {
    if value.0.is_finite() {
        rems(value.0.max(0.))
    } else {
        current
    }
}

#[derive(Clone, Copy)]
pub(super) struct ResolvedListLayout {
    pub(super) lane: Pixels,
    pub(super) marker_gap: Pixels,
    pub(super) item_gap: Pixels,
    pub(super) nested_gap: Pixels,
    pub(super) paragraph_gap: Pixels,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TextViewStyle;

    #[test]
    fn geometry_is_opt_in_and_every_length_participates_in_style_equality() {
        let neutral = TextViewStyle::default();
        assert!(neutral.list_layout().is_none());
        let layout = ListLayout::new();
        let styled = neutral.clone().with_list_layout(layout);
        assert!(neutral != styled);
        assert_eq!(styled.list_layout(), Some(&layout));
        for changed in [
            layout.with_indent(rems(2.)),
            layout.with_marker_gap(rems(0.4)),
            layout.with_item_gap(rems(0.5)),
            layout.with_loose_item_gap(rems(1.)),
            layout.with_paragraph_gap(rems(1.25)),
        ] {
            assert!(styled != neutral.clone().with_list_layout(changed));
        }
    }

    #[test]
    fn defaults_and_length_sanitization_are_stable() {
        let layout = ListLayout::new();
        assert_eq!(layout.indent(), rems(1.4));
        assert_eq!(layout.marker_gap(), rems(0.3));
        assert_eq!(layout.item_gap(), rems(0.25));
        assert_eq!(layout.loose_item_gap(), rems(0.75));
        assert_eq!(layout.paragraph_gap(), rems(0.75));
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                layout
                    .with_indent(rems(value))
                    .with_marker_gap(rems(value))
                    .with_item_gap(rems(value))
                    .with_loose_item_gap(rems(value))
                    .with_paragraph_gap(rems(value)),
                layout
            );
        }
        let zero = layout
            .with_indent(rems(-1.))
            .with_marker_gap(rems(-2.))
            .with_item_gap(rems(-3.))
            .with_loose_item_gap(rems(-4.))
            .with_paragraph_gap(rems(-5.));
        assert_eq!(zero.indent(), rems(0.));
        assert_eq!(zero.marker_gap(), rems(0.));
        assert_eq!(zero.item_gap(), rems(0.));
        assert_eq!(zero.loose_item_gap(), rems(0.));
        assert_eq!(zero.paragraph_gap(), rems(0.));
    }
}
