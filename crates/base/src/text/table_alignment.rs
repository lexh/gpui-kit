use super::{
    TextViewStyle,
    node::{ColumnumnAlign, Table},
};

impl Table {
    /// Alignment is a presentation decision, never written back to the AST.
    pub(super) fn effective_column_alignments(&self, style: &TextViewStyle) -> Vec<ColumnumnAlign> {
        let columns = self
            .children
            .iter()
            .map(|row| row.children.len())
            .max()
            .unwrap_or(0)
            .max(self.column_aligns.len());
        (0..columns)
            .map(|ix| {
                let source = self.column_align(ix);
                if source != ColumnumnAlign::None || !style.has_numeric_table_alignment() {
                    return source;
                }
                let mut has_value = false;
                for row in self.children.iter().skip(1) {
                    let Some(cell) = row.children.get(ix) else {
                        continue;
                    };
                    if cell
                        .children
                        .children
                        .iter()
                        .any(|node| node.image.is_some() || node.custom.is_some())
                    {
                        return source;
                    }
                    let text = cell.children.text();
                    let text = text.trim_matches([' ', '\t']);
                    if text.is_empty() {
                        continue;
                    }
                    if !is_numeric(text) {
                        return source;
                    }
                    has_value = true;
                }
                if has_value {
                    ColumnumnAlign::Right
                } else {
                    source
                }
            })
            .collect()
    }
}

/// A deliberately permissive display heuristic, not a number parser.
/// Keep the ASCII number/unit grammar shared with the print column filter.
fn is_numeric(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut ix = usize::from(bytes.first() == Some(&b'~'));
    if matches!(bytes.get(ix), Some(b'+' | b'-')) {
        ix += 1;
    }
    if !bytes.get(ix).is_some_and(u8::is_ascii_digit) {
        return false;
    }
    ix += 1;
    while bytes
        .get(ix)
        .is_some_and(|byte| byte.is_ascii_digit() || b".,/:%".contains(byte))
    {
        ix += 1;
    }
    while matches!(bytes.get(ix), Some(b' ' | b'\t')) {
        ix += 1;
    }
    let unit = &bytes[ix..];
    unit.len() <= 3 && unit.iter().all(u8::is_ascii_alphabetic)
}

#[cfg(test)]
mod tests {
    use crate::text::{
        TextViewStyle,
        format::markdown,
        node::{BlockNode, ColumnumnAlign as Align, NodeContext},
    };

    fn table(source: &str) -> crate::text::node::Table {
        let document = markdown::parse(source, &mut NodeContext::default()).expect("parse table");
        let [BlockNode::Table(table)] = document.blocks.as_slice() else {
            panic!("expected one table");
        };
        table.clone()
    }

    #[test]
    fn numeric_alignment_is_opt_in_and_never_changes_source_alignments() {
        let table = table(
            "| Quantity | Left | Center | Right | Mixed |\n\
             | --- | :--- | :---: | ---: | --- |\n\
             | **42** | 42 | 42 | text | 42 |\n\
             | `142ms` | 7 | 7 | other | n/a |",
        );
        let neutral = TextViewStyle::default();
        let inferred = neutral.clone().with_numeric_table_alignment(true);
        let source_alignments = table.column_aligns().to_vec();
        let source = table.to_markdown();
        let snapshot = table.table_data();

        assert!(!neutral.has_numeric_table_alignment());
        assert!(inferred.has_numeric_table_alignment());
        assert!(neutral != inferred);
        assert_eq!(
            table.effective_column_alignments(&neutral),
            source_alignments
        );
        assert_eq!(
            table.effective_column_alignments(&inferred),
            [
                Align::Right,
                Align::Left,
                Align::Center,
                Align::Right,
                Align::None
            ]
        );
        assert_eq!(table.column_aligns(), source_alignments);
        assert_eq!(table.to_markdown(), source);
        assert_eq!(table.table_data(), snapshot);
        assert_eq!(
            table.effective_column_alignments(&inferred.with_numeric_table_alignment(false)),
            source_alignments
        );
    }

    #[test]
    fn numeric_columns_ignore_headers_and_empty_or_missing_body_cells() {
        let style = TextViewStyle::default().with_numeric_table_alignment(true);
        for (source, expected) in [
            (
                "| Prose heading |\n| --- |\n| 3 |\n| |\n| 10/12 |",
                vec![Align::Right],
            ),
            ("| 123 |\n| --- |", vec![Align::None]),
            ("| 123 |\n| --- |\n| |", vec![Align::None]),
            (
                "| A | B |\n| --- | --- |\n| 12 | 42 |\n| 9 |",
                vec![Align::Right, Align::Right],
            ),
            (
                "| A | B |\n| --- | --- |\n| 12 | |\n| 9 |",
                vec![Align::Right, Align::None],
            ),
        ] {
            assert_eq!(
                table(source).effective_column_alignments(&style),
                expected,
                "{source}"
            );
        }
    }

    #[test]
    fn inline_objects_and_multiline_values_are_not_numeric() {
        let style = TextViewStyle::default().with_numeric_table_alignment(true);
        for value in ["![badge](badge.png)", "42 ![badge](badge.png)", "42<br>ms"] {
            let source = format!("| Value |\n| --- |\n| 12 |\n| {value} |");
            assert_eq!(
                table(&source).effective_column_alignments(&style),
                [Align::None],
                "{value}"
            );
        }
        for text in ["", "42"] {
            let mut table = table("| Value |\n| --- |\n| 12 |\n| 42 |");
            table.children[2].children[0].children.children =
                vec![crate::text::node::InlineNode::custom(
                    crate::text::MarkdownNode::new("value", ()).text(text),
                )];
            assert_eq!(table.effective_column_alignments(&style), [Align::None]);
        }
    }

    #[test]
    fn numeric_values_match_short_ascii_units_and_the_print_filter_grammar() {
        let style = TextViewStyle::default().with_numeric_table_alignment(true);
        for value in [
            "3",
            "10/12",
            "-73.2%",
            "1,200 rps",
            "142ms",
            "~+42",
            "+8",
            "1:30",
            "1.2.3",
            "12 / 30",
        ] {
            let source = format!("| Value |\n| --- |\n| {value} |");
            let expected = if value == "12 / 30" {
                Align::None
            } else {
                Align::Right
            };
            assert_eq!(
                table(&source).effective_column_alignments(&style),
                [expected],
                "{value}"
            );
        }
        for value in [
            "",
            "~",
            ".5",
            "NaN",
            "42 requests",
            "1e3",
            "$12",
            "42 ms later",
            "12/30 apples",
            "42m/s",
            "42\u{00b5}s",
            "\u{2212}12",
        ] {
            let source = format!("| Value |\n| --- |\n| {value} |");
            assert_eq!(
                table(&source).effective_column_alignments(&style),
                [Align::None],
                "{value}"
            );
        }
    }
}
