use super::*;
use crate::text::inline::test_fonts::{BODY, WideMonoTextSystem};
use gpui::TestApp;

fn in_window(test: impl FnOnce(&mut Window, &mut App) + 'static) {
    let mut app = TestApp::with_text_system(Arc::new(WideMonoTextSystem));
    crate::text::inline::test_draw::in_prepaint(&mut app, test);
}

fn style() -> TextStyle {
    TextStyle {
        font_family: BODY.into(),
        font_size: px(16.).into(),
        ..Default::default()
    }
}

fn text(text: &str, highlights: Vec<(Range<usize>, InlineHighlight)>) -> MeasureItem {
    MeasureItem::Text {
        text: text.to_string().into(),
        links: vec![],
        highlights,
    }
}

fn hanging(prefix_len: usize, lane: f32) -> FlowHanging {
    FlowHanging {
        prefix_len,
        min_indent: px(lane),
        labels: None,
    }
}

fn layout(
    items: &[MeasureItem],
    width: f32,
    hanging: &FlowHanging,
    window: &mut Window,
    cx: &mut App,
) -> InlineFlowLayout {
    let style = style();
    let objects = prepare_objects(items, &style, window, cx);
    layout_measured_flow(
        items,
        &vec![None; items.len()],
        &objects,
        &style,
        Some(px(width)),
        Some(hanging),
        window,
        cx,
    )
}

#[test]
fn footnote_hanging_prefix_is_atomic_and_every_body_row_shares_its_lane() {
    in_window(|window, cx| {
        let prefix = "[a-long-label]: ";
        let value = format!("{prefix}alpha beta gamma delta epsilon zeta eta theta");
        let labels = Arc::new(vec![prefix.into(), "[short]: ".into()]);
        let hanging = FlowHanging {
            prefix_len: prefix.len(),
            min_indent: px(0.),
            labels: Some(labels),
        };
        let items = [text(&value, vec![])];
        let layout = layout(&items, 240., &hanging, window, cx);
        let lane = WideMonoTextSystem::width_of(prefix, BODY, px(16.));
        let mut rows: Vec<(Pixels, Pixels)> = Vec::new();
        let mut prefix_count = 0;
        let mut copied = String::new();
        for fragment in &layout.fragments {
            if let PositionedFragment::Text {
                text,
                source_range,
                origin,
                selection_bounds,
                ..
            } = fragment
            {
                copied.push_str(text);
                assert_eq!(&value[source_range.clone()], text.as_ref());
                if source_range.start == 0 {
                    assert_eq!(text.as_ref(), prefix);
                    prefix_count += 1;
                } else {
                    if rows
                        .last()
                        .is_none_or(|row| row.0 != selection_bounds.top())
                    {
                        rows.push((selection_bounds.top(), origin.x));
                    }
                    assert!(origin.x >= lane);
                    assert!(selection_bounds.right() <= px(240.));
                }
            }
        }
        assert_eq!(copied, value);
        assert_eq!(prefix_count, 1);
        assert!(rows.len() > 2);
        assert!(rows.iter().all(|row| row.1 == lane));
        assert!(layout.size.width <= px(240.));
    });
}

#[test]
fn footnote_empty_continuations_and_too_narrow_lanes_stay_finite() {
    in_window(|window, cx| {
        let items = [text("[a]: \n\n", vec![])];
        let layout = layout(&items, 100., &hanging(5, 60.), window, cx);
        assert_eq!(layout.empty_lines.len(), 2);
        assert!(
            layout
                .empty_lines
                .iter()
                .all(|(_, bounds)| bounds.left() == px(60.))
        );
        for width in [1., 40., 60.] {
            let items = [text("[a]: body words", vec![])];
            let layout = self::layout(&items, width, &hanging(5, 60.), window, cx);
            let PositionedFragment::Text { origin, .. } = &layout.fragments[0] else {
                panic!("text")
            };
            assert_eq!(origin.x, px(0.), "non-hanging fallback at {width}");
            assert!(layout.size.width.as_f32().is_finite());
            assert!(layout.size.height > px(0.));
        }
    });
}

#[test]
fn footnote_image_and_custom_text_fit_the_same_positive_body_budget() {
    in_window(|window, cx| {
        let style = style();
        let items = [
            text("[a]: ", vec![]),
            MeasureItem::Image {
                source: gpui::SharedUri::from("https://example.com/image.png").into(),
                width: None,
                height: None,
            },
            MeasureItem::Object {
                text: "a long fallback object with several words".into(),
                id: 0,
                renderer: Arc::new(|_, _, _| None),
                style: Default::default(),
            },
        ];
        let objects = prepare_objects(&items, &style, window, cx);
        let layout = layout_measured_flow(
            &items,
            &[None, Some(size(px(500.), px(50.))), None],
            &objects,
            &style,
            Some(px(180.)),
            Some(&hanging(5, 60.)),
            window,
            cx,
        );
        let image = layout
            .fragments
            .iter()
            .find_map(|fragment| match fragment {
                PositionedFragment::Image { origin, size, .. } => Some((*origin, *size)),
                _ => None,
            })
            .unwrap();
        assert_eq!(image.0.x, px(60.));
        assert_eq!(image.1, size(px(120.), px(12.)));
        let object = layout
            .fragments
            .iter()
            .find_map(|fragment| match fragment {
                PositionedFragment::Object { origin, object, .. } => {
                    Some((*origin, &object.metrics))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(object.0.x, px(60.));
        assert!(object.1.size.width <= px(120.));
        assert!(layout.size.width <= px(180.));
    });
}

#[test]
fn footnote_code_first_body_wraps_inside_its_reserved_lane() {
    in_window(|window, cx| {
        let value = "[a]: a_long_code_span_with_more_than_one_line";
        let items = [text(
            value,
            vec![(
                5..value.len(),
                InlineHighlight {
                    font_size_scale: Some(0.875),
                    font_family: Some(BODY.into()),
                    code_style: Some(StyleRefinement::default()),
                    ..Default::default()
                },
            )],
        )];
        let layout = layout(&items, 140., &hanging(5, 60.), window, cx);
        assert!(layout.fragments.len() > 2);
        let mut copied = String::new();
        for fragment in &layout.fragments {
            if let PositionedFragment::Text {
                text,
                source_range,
                selection_bounds,
                ..
            } = fragment
            {
                copied.push_str(text);
                if source_range.start >= 5 {
                    assert!(selection_bounds.left() >= px(60.));
                    assert!(selection_bounds.right() <= px(140.));
                }
            }
        }
        assert_eq!(copied, value);
    });
}

#[test]
fn footnote_reference_raise_is_applied_once_and_never_adds_code_padding() {
    in_window(|window, cx| {
        for body_size in [16., 20., 24.] {
            for raise in [0., 0.3, 1.5] {
                let style = TextStyle {
                    font_size: px(body_size).into(),
                    ..style()
                };
                let items = [text(
                    "x[a] y",
                    vec![(
                        1..4,
                        InlineHighlight {
                            font_size_scale: Some(0.7),
                            baseline_raise: Some(raise),
                            ..Default::default()
                        },
                    )],
                )];
                let layout = layout_flow(&items, &[None], &style, None, window, cx);
                let mut baselines = Vec::new();
                let mut row = None;
                for fragment in &layout.fragments {
                    if let PositionedFragment::Text {
                        text,
                        font_size,
                        origin,
                        size,
                        selection_bounds,
                        code_background,
                        ..
                    } = fragment
                    {
                        let shaped = shape_line(
                            text.clone(),
                            *font_size,
                            &text_runs(text.len(), &style, &[]),
                            window,
                        );
                        baselines.push(
                            origin.y
                                + (size.height - shaped.ascent - shaped.descent) / 2.
                                + shaped.ascent,
                        );
                        if let Some(row) = row {
                            assert_eq!(
                                *selection_bounds,
                                Bounds::new(
                                    point(selection_bounds.left(), row),
                                    selection_bounds.size
                                )
                            );
                        }
                        row = Some(selection_bounds.top());
                        assert!(code_background.is_none());
                        if text.as_ref() == "[a]" {
                            assert_eq!(
                                size.width,
                                WideMonoTextSystem::width_of("[a]", BODY, px(body_size * 0.7))
                            );
                        }
                    }
                }
                assert_eq!(baselines.len(), 3);
                assert!((baselines[0] - baselines[1] - px(body_size * raise)).abs() < px(0.001));
                assert_eq!(baselines[0], baselines[2]);
            }
        }
    });
}

#[test]
fn footnote_scaled_reference_wrapping_uses_the_shaped_non_code_width() {
    in_window(|window, cx| {
        let value = "left[a]right tail";
        let items = [text(
            value,
            vec![(
                4..7,
                InlineHighlight {
                    font_size_scale: Some(0.5),
                    ..Default::default()
                },
            )],
        )];
        let width = WideMonoTextSystem::width_of("leftright", BODY, px(16.))
            + WideMonoTextSystem::width_of("[a]", BODY, px(8.));
        let layout = layout_flow(&items, &[None], &style(), Some(width), window, cx);
        let first: String = layout
            .fragments
            .iter()
            .filter_map(|fragment| match fragment {
                PositionedFragment::Text {
                    text,
                    selection_bounds,
                    ..
                } if selection_bounds.top() == px(0.) => Some(text.as_ref()),
                _ => None,
            })
            .collect();
        assert_eq!(first.trim_end(), "left[a]right");
        assert!(layout.size.height > plain_line_height(&style(), window.rem_size(), window));
    });
}
