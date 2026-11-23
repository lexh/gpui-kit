use gpui::{
    AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Pixels, Point, Render, StyleRefinement, Styled as _, TestAppContext,
    VisualTestContext, Window, div, point, px, rems,
};

use super::{ListLayout, TextView, TextViewState, TextViewStyle};

struct ListRoot {
    state: Entity<TextViewState>,
    layout: ListLayout,
    width: Pixels,
    marker_style: StyleRefinement,
}

impl Render for ListRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(self.width)
            .text_size(px(16.))
            .underline()
            .child(crate::TextSelectionLayer)
            .child(
                TextView::new(&self.state)
                    .style(
                        TextViewStyle::default()
                            .with_list_layout(self.layout)
                            .with_list_style(self.marker_style.clone()),
                    )
                    .code_block_actions(|_, _, _| {
                        div()
                            .debug_selector(|| "list-code-actions".into())
                            .child("Copy")
                    }),
            )
    }
}

fn mount<'a>(
    source: &str,
    layout: ListLayout,
    width: f32,
    cx: &'a mut TestAppContext,
) -> (Entity<ListRoot>, &'a mut VisualTestContext) {
    cx.update(crate::init);
    let (root, cx) = cx.add_window_view(|_, cx| ListRoot {
        state: cx.new(|cx| TextViewState::markdown(source, cx)),
        layout,
        width: px(width),
        marker_style: StyleRefinement::default(),
    });
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear(cx));
    (root, cx)
}

fn position(
    root: &Entity<ListRoot>,
    text: &str,
    offset: usize,
    cx: &mut VisualTestContext,
) -> Point<Pixels> {
    root.read_with(cx, |root, cx| {
        root.state
            .read(cx)
            .selection_adapter
            .text_position(text, offset)
    })
}

#[gpui::test]
fn list_layout_shares_task_bullet_and_continuation_spines(cx: &mut TestAppContext) {
    let source = "- Bullet\n- [ ] Task\n- [x] Done\n- Tail\n\n  Continue\n\n  - Nested\n  - [ ] Child\n\n  After\n- Last";
    let (root, cx) = mount(source, ListLayout::new(), 180., cx);
    let spine = position(&root, "Bullet", 0, cx).x;
    for text in ["Task", "Done", "Tail", "Continue", "After", "Last"] {
        assert_eq!(
            position(&root, text, 0, cx).x,
            spine,
            "{text} must share its list's spine"
        );
    }
    let nested = position(&root, "Nested", 0, cx).x;
    assert!(nested > spine);
    assert_eq!(nested, position(&root, "Child", 0, cx).x);

    let start = position(&root, "Continue", 0, cx) + point(px(0.1), px(0.));
    let end = position(&root, "Continue", "Continue".len(), cx) - point(px(0.1), px(0.));
    cx.simulate_mouse_down(start, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_mouse_move(end, MouseButton::Left, Modifiers::default());
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.simulate_mouse_up(end, MouseButton::Left, Modifiers::default());
    root.read_with(cx, |root, cx| {
        let state = root.state.read(cx);
        assert_eq!(state.selected_text(), "Continue\n");
        let selected = state.selected_source_range().expect("source mapping");
        assert_eq!(&source[selected], "Continue");
        assert_eq!(state.rendered_text().source(), source);
    });
}

#[gpui::test]
fn list_layout_widens_one_lane_for_large_ordinals_and_constrained_bodies(cx: &mut TestAppContext) {
    let source = "98. Apple\n99. Banana\n100. Cherry\n101. Date";
    let (root, cx) = mount(source, ListLayout::new().with_indent(rems(0.)), 130., cx);
    let spine = position(&root, "Apple", 0, cx).x;
    assert!(spine > px(0.));
    for text in ["Banana", "Cherry", "Date"] {
        assert_eq!(position(&root, text, 0, cx).x, spine);
    }
    root.read_with(cx, |root, cx| {
        for bounds in root.state.read(cx).selection_adapter.text_bounds() {
            assert!(bounds.left() >= px(0.) && bounds.right() <= root.width);
        }
    });
    root.update(cx, |root, cx| {
        root.marker_style = StyleRefinement::default().text_size(px(24.));
        root.width = px(300.);
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    let wider = position(&root, "Apple", 0, cx).x;
    assert!(
        wider > spine,
        "the shared lane must remeasure refined marker text"
    );
    assert_eq!(position(&root, "Cherry", 0, cx).x, wider);
}

#[gpui::test]
fn list_layout_wraps_bodies_and_preserves_code_first_items(cx: &mut TestAppContext) {
    for width in [130., 500.] {
        let source = "- [ ] A long task body with several words that must wrap inside the view without clipping.\n- Another long body that stays within its available width.";
        let (root, vcx) = mount(source, ListLayout::new(), width, cx);
        root.read_with(vcx, |root, cx| {
            for bounds in root.state.read(cx).selection_adapter.text_bounds() {
                assert!(bounds.left() >= px(0.) && bounds.right() <= root.width);
            }
        });
    }
    let (_, cx) = mount(
        "- ```rust\n  code\n  ```\n- Body",
        ListLayout::new(),
        300.,
        cx,
    );
    let code = cx
        .debug_bounds("list-code-actions")
        .expect("code-first item is rendered");
    assert!(code.left() > px(0.) && code.right() <= px(300.));
}

#[gpui::test]
fn list_layout_keeps_a_non_task_marker_under_a_task(cx: &mut TestAppContext) {
    let (_, cx) = mount("- [ ] Parent\n  - Child", ListLayout::new(), 300., cx);
    cx.update(|window, _| {
        assert_eq!(
            window.painted_underlines().len(),
            3,
            "parent body, child body and child marker"
        );
    });
}

#[gpui::test]
fn list_layout_loose_lists_and_continuations_use_distinct_gaps(cx: &mut TestAppContext) {
    let layout = ListLayout::new()
        .with_item_gap(rems(0.))
        .with_loose_item_gap(rems(1.));
    let gap = |source: &str, cx: &mut TestAppContext| {
        let (root, cx) = mount(source, layout, 300., cx);
        position(&root, "Second", 0, cx).y - position(&root, "First", 0, cx).y
    };
    let tight = gap("- First\n- Second", cx);
    let loose = gap("- First\n\n- Second", cx);
    assert!(loose > tight, "a source blank line makes the list loose");

    let continuation_gap = |paragraph_gap, cx: &mut TestAppContext| {
        let (root, cx) = mount(
            "- First\n\n  - Child\n\n  After",
            layout.with_paragraph_gap(paragraph_gap),
            300.,
            cx,
        );
        position(&root, "After", 0, cx).y - position(&root, "Child", 0, cx).y
    };
    assert!(continuation_gap(rems(1.), cx) > continuation_gap(rems(0.), cx));
}
