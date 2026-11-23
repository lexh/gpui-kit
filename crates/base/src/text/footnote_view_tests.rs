use gpui::{
    AppContext as _, Context, Entity, IntoElement, Modifiers, MouseButton, ParentElement as _,
    Pixels, Point, Render, Styled as _, TestAppContext, VisualTestContext, Window, div, px, rems,
};

use super::{FootnoteStyle, RangeHighlight, TextView, TextViewState, TextViewStyle};

struct FootnoteRoot {
    state: Entity<TextViewState>,
    style: Option<FootnoteStyle>,
    width: Pixels,
    scrollable: bool,
}

impl Render for FootnoteRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut style = TextViewStyle::default()
            .with_paragraph_gap(rems(1.))
            .with_blockquote_style(gpui::StyleRefinement::default().text_size(px(20.)));
        if let Some(footnotes) = self.style {
            style = style.with_footnote_style(footnotes);
        }
        div()
            .w(self.width)
            .h(px(300.))
            .overflow_hidden()
            .text_size(px(16.))
            .child(crate::TextSelectionLayer)
            .child(
                TextView::new(&self.state)
                    .style(style)
                    .scrollable(self.scrollable),
            )
    }
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.run_until_parked();
}

fn mount<'a>(
    source: &str,
    scrollable: bool,
    cx: &'a mut TestAppContext,
) -> (Entity<FootnoteRoot>, &'a mut VisualTestContext) {
    cx.update(crate::init);
    let (root, cx) = cx.add_window_view(|_, cx| FootnoteRoot {
        state: cx.new(|cx| TextViewState::markdown(source, cx)),
        style: Some(
            FootnoteStyle::new()
                .with_reference_scale(0.72)
                .with_reference_raise(0.3)
                .with_entry_scale(0.8)
                .with_indent(rems(2.)),
        ),
        width: px(360.),
        scrollable,
    });
    settle(cx);
    (root, cx)
}

fn position(
    root: &Entity<FootnoteRoot>,
    text: &str,
    index: usize,
    cx: &mut VisualTestContext,
) -> Point<Pixels> {
    root.read_with(cx, |root, cx| {
        root.state
            .read(cx)
            .selection_adapter
            .text_position(text, index)
    })
}

#[gpui::test]
fn footnote_native_entries_share_a_spine_and_keep_source_order_and_following_gaps(
    cx: &mut TestAppContext,
) {
    let source = "Body[^a].\n\n[^a]: Alpha\n\n[^long-label]: Beta\n\nMain follows.\n\n[^c]: Omega";
    let (root, cx) = mount(source, false, cx);
    let alpha = position(&root, "Alpha", 0, cx);
    let beta = position(&root, "Beta", 0, cx);
    let main = position(&root, "Main follows.", 0, cx);
    let omega = position(&root, "Omega", 0, cx);
    assert!((alpha.x - beta.x).abs() < px(0.001));
    assert!((alpha.x - omega.x).abs() < px(0.001));
    assert!(alpha.y < beta.y && beta.y < main.y && main.y < omega.y);
    let preceding_prose_gap = main.y - beta.y;
    let ordinary_entry_distance = beta.y - alpha.y;
    root.update(cx, |root, cx| {
        root.style = root.style.map(|s| s.with_entry_gap(rems(0.1)));
        cx.notify();
    });
    settle(cx);
    let alpha = position(&root, "Alpha", 0, cx);
    let beta = position(&root, "Beta", 0, cx);
    let main = position(&root, "Main follows.", 0, cx);
    assert!(beta.y - alpha.y < ordinary_entry_distance);
    assert!((main.y - beta.y - preceding_prose_gap).abs() < px(0.001));
    root.read_with(cx, |root, cx| {
        assert_eq!(root.state.read(cx).rendered_text().source(), source)
    });
}

#[gpui::test]
fn footnote_native_marker_body_drag_and_search_survive_width_and_style_reflow(
    cx: &mut TestAppContext,
) {
    const SEARCH: u32 = 0xc196f5;
    let source = "Body[^a] and [ordinary][a].\n\n[^a]: Alpha\n\n[a]: https://example.com";
    let (root, cx) = mount(source, false, cx);
    let start = root.read_with(cx, |root, cx| {
        root.state
            .read(cx)
            .rendered_text()
            .as_str()
            .find("Alpha")
            .unwrap()
    });
    root.update(cx, |root, cx| {
        root.state.update(cx, |state, cx| {
            state
                .set_range_highlights(
                    [RangeHighlight::new(start..start + 5, gpui::rgb(SEARCH))],
                    cx,
                )
                .unwrap();
        })
    });
    settle(cx);
    let first = position(&root, "[a]: ", 1, cx);
    let last = position(&root, "Alpha", 5, cx);
    cx.simulate_mouse_down(first, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_mouse_move(last, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_mouse_up(last, MouseButton::Left, Modifiers::default());
    settle(cx);
    let (selected, rendered) = root.read_with(cx, |root, cx| {
        let state = root.state.read(cx);
        assert_eq!(
            &source[state.selected_source_range().unwrap()],
            "[^a]: Alpha"
        );
        assert!(state.selected_text().contains("Alpha"));
        (
            state.selected_text(),
            state.rendered_text().as_str().to_string(),
        )
    });
    for enabled in [true, false, true] {
        root.update(cx, |root, cx| {
            root.width = px(220.);
            root.style = enabled.then(|| {
                FootnoteStyle::new()
                    .with_entry_scale(0.7)
                    .with_indent(rems(3.))
            });
            cx.notify();
        });
        settle(cx);
        root.read_with(cx, |root, cx| {
            let state = root.state.read(cx);
            assert_eq!(state.selected_text(), selected);
            assert_eq!(
                &source[state.selected_source_range().unwrap()],
                "[^a]: Alpha"
            );
            assert_eq!(state.rendered_text().as_str(), rendered);
            assert_eq!(state.rendered_text().source(), source);
        });
        cx.update(|window, _| {
            let search: gpui::Background = gpui::rgb(SEARCH).into();
            assert!(
                window
                    .painted_quads()
                    .iter()
                    .any(|quad| quad.background == search)
            );
        });
    }
}

#[gpui::test]
fn footnote_native_repeated_text_search_and_copy_target_the_exact_definition(
    cx: &mut TestAppContext,
) {
    const SEARCH: u32 = 0xb396e8;
    let source = "Alpha[^a].\n\n[^a]: Alpha\n\n[^b]: Alpha";
    let (root, cx) = mount(source, false, cx);
    let rendered_start = root.read_with(cx, |root, cx| {
        root.state
            .read(cx)
            .rendered_text()
            .as_str()
            .match_indices("Alpha")
            .nth(1)
            .unwrap()
            .0
    });
    root.update(cx, |root, cx| {
        root.state.update(cx, |state, cx| {
            state
                .set_range_highlights(
                    [RangeHighlight::new(
                        rendered_start..rendered_start + 5,
                        gpui::rgb(SEARCH),
                    )],
                    cx,
                )
                .unwrap()
        })
    });
    settle(cx);
    let boxes = cx.update(|window, _| {
        let color: gpui::Background = gpui::rgb(SEARCH).into();
        let scale = window.scale_factor();
        window
            .painted_quads()
            .into_iter()
            .filter(|quad| quad.background == color)
            .map(|quad| quad.bounds.map(|value| px(value.0 / scale)))
            .collect::<Vec<_>>()
    });
    assert_eq!(boxes.len(), 1);
    let bounds = boxes[0];
    let marker = position(&root, "[a]: ", 0, cx);
    assert!((bounds.center().y - marker.y).abs() < px(1.));
    let start = gpui::point(bounds.left() + px(0.1), bounds.center().y);
    let end = gpui::point(bounds.right() - px(0.1), bounds.center().y);
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    settle(cx);
    let expected = source.find("[^a]: Alpha").unwrap() + "[^a]: ".len();
    root.read_with(cx, |root, cx| {
        let state = root.state.read(cx);
        assert_eq!(state.selected_text().trim(), "Alpha");
        assert_eq!(state.selected_source_range(), Some(expected..expected + 5));
    });
    assert_eq!(
        cx.update(|window, cx| crate::TextSelection::selected_text(window, cx))
            .trim(),
        "Alpha"
    );
}

#[gpui::test]
fn footnote_native_unchanged_frames_reuse_flow_layouts(cx: &mut TestAppContext) {
    let (root, cx) = mount("Body[^a].\n\n[^a]: Alpha", false, cx);
    let count = super::inline_flow::FLOW_LAYOUTS.with(|n| n.get());
    for _ in 0..3 {
        cx.update(|window, cx| window.draw(cx).clear(cx));
    }
    assert_eq!(super::inline_flow::FLOW_LAYOUTS.with(|n| n.get()), count);
    root.update(cx, |root, cx| {
        root.style = root.style.map(|s| s.with_indent(rems(5.)));
        cx.notify();
    });
    settle(cx);
    assert!(super::inline_flow::FLOW_LAYOUTS.with(|n| n.get()) > count);
}

#[gpui::test]
fn footnote_native_wrapped_marker_and_body_drag_in_both_directions(cx: &mut TestAppContext) {
    let source = "[^a]: Alpha  \n    Beta";
    let (root, cx) = mount(source, false, cx);
    for reverse in [false, true] {
        let first = position(&root, "[a]: ", 1, cx);
        let last = position(&root, "Beta", 4, cx);
        assert!(last.y > first.y);
        let (start, end) = if reverse {
            (last, first)
        } else {
            (first, last)
        };
        cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
        settle(cx);
        root.read_with(cx, |root, cx| {
            let state = root.state.read(cx);
            assert_eq!(&source[state.selected_source_range().unwrap()], source);
            assert!(state.selected_text().contains("Alpha\nBeta"));
        });
    }
}

#[gpui::test]
fn footnote_native_entry_size_inherits_its_quote_container(cx: &mut TestAppContext) {
    let (root, cx) = mount("[^a]: Alpha\n\n> [^b]: Beta", false, cx);
    let root_width = position(&root, "[a]: ", 5, cx).x - position(&root, "[a]: ", 0, cx).x;
    let quote_width = position(&root, "[b]: ", 5, cx).x - position(&root, "[b]: ", 0, cx).x;
    assert!((quote_width / root_width - 20. / 16.).abs() < 0.001);
}

#[gpui::test]
fn footnote_native_entry_size_also_applies_inside_list_items(cx: &mut TestAppContext) {
    let (root, cx) = mount("[^a]: Root\n\n- [^b]: List", false, cx);
    let root_width = position(&root, "[a]: ", 5, cx).x - position(&root, "[a]: ", 0, cx).x;
    let list_width = position(&root, "[b]: ", 5, cx).x - position(&root, "[b]: ", 0, cx).x;
    assert!((root_width - list_width).abs() < px(0.001));
}

#[gpui::test]
fn footnote_native_table_columns_measure_scaled_reference_runs(cx: &mut TestAppContext) {
    let source = "| H | Other |\n| - | - |\n| A[^label] | Tail |\n\n[^label]: Entry";
    let (root, cx) = mount(source, false, cx);
    let before = position(&root, "Tail", 0, cx).x;
    root.update(cx, |root, cx| {
        root.style = root.style.map(|s| s.with_reference_scale(2.));
        cx.notify();
    });
    settle(cx);
    assert!(position(&root, "Tail", 0, cx).x > before + px(1.));
    root.read_with(cx, |root, cx| {
        for bounds in root.state.read(cx).selection_adapter.text_bounds() {
            assert!(bounds.left() >= px(0.) && bounds.right() <= root.width);
        }
    });
}

fn append_lane_regression(grow_count: bool, cx: &mut TestAppContext) {
    let source = format!(
        "[^a]: {}\n\n{}\n\n{}",
        "body words ".repeat(100),
        (0..40)
            .map(|ix| format!("Middle {ix}"))
            .collect::<Vec<_>>()
            .join("\n\n"),
        if grow_count {
            "Tail"
        } else {
            "[^a-longer-label]"
        }
    );
    let (root, cx) = mount(&source, true, cx);
    let extent = root.read_with(cx, |root, cx| {
        root.state
            .read(cx)
            .list_state()
            .max_offset_for_scrollbar()
            .y
    });
    root.read_with(cx, |root, cx| {
        root.state
            .read(cx)
            .list_state()
            .scroll_to(gpui::ListOffset {
                item_ix: 20,
                offset_in_item: px(0.),
            })
    });
    settle(cx);
    let anchor = root.read_with(cx, |root, cx| {
        root.state.read(cx).list_state().logical_scroll_top()
    });
    assert_eq!(anchor.item_ix, 20, "the earlier entry must be offscreen");
    let count = root.read_with(cx, |root, cx| root.state.read(cx).list_state().item_count());
    root.update(cx, |root, cx| {
        root.state.update(cx, |state, cx| {
            state.push_str(
                if grow_count {
                    "\n\n[^a-longer-label]: Final"
                } else {
                    ": Final"
                },
                cx,
            )
        })
    });
    settle(cx);
    root.read_with(cx, |root, cx| {
        let list = root.state.read(cx).list_state();
        assert_eq!(list.item_count(), count + usize::from(grow_count));
        assert_eq!(list.logical_scroll_top(), anchor);
        assert!(
            list.max_offset_for_scrollbar().y > extent + px(100.),
            "earlier entry must remeasure when the shared label lane grows"
        );
    });
    root.update(cx, |root, cx| {
        root.state
            .update(cx, |state, cx| state.set_text(&source, cx))
    });
    settle(cx);
    root.read_with(cx, |root, cx| {
        let list = root.state.read(cx).list_state();
        assert_eq!(list.item_count(), count);
        assert!(
            (list.max_offset_for_scrollbar().y - extent).abs() < px(1.),
            "removing the wider label must shrink earlier entries again"
        );
    });
}

#[gpui::test]
fn footnote_native_grown_count_append_remeasures_earlier_entries_without_moving_anchor(
    cx: &mut TestAppContext,
) {
    append_lane_regression(true, cx);
}

#[gpui::test]
fn footnote_native_same_count_append_remeasures_earlier_entries_without_moving_anchor(
    cx: &mut TestAppContext,
) {
    append_lane_regression(false, cx);
}
