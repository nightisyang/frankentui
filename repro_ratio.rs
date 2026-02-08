#![forbid(unsafe_code)]

use ftui_layout::{Constraint, Flex, Rect};

#[test]
fn percentage_is_absolute_ratio_is_weighted() {
    let area = Rect::new(0, 0, 100, 10);

    // Case 1: Percentage(25) alone.
    let flex_p = Flex::horizontal().constraints([Constraint::Percentage(25.0)]);
    let rects_p = flex_p.split(area);
    assert_eq!(rects_p[0].width, 25, "Percentage(25) should take 25%");

    // Case 2: Ratio(1, 4) alone.
    let flex_r = Flex::horizontal().constraints([Constraint::Ratio(1, 4)]);
    let rects_r = flex_r.split(area);

    assert_eq!(
        rects_r[0].width, 100,
        "A lone Ratio(1, 4) is a grow item, so it takes all available space (got {})",
        rects_r[0].width
    );
}

#[test]
fn ratio_is_weighted_against_other_grow_items() {
    let area = Rect::new(0, 0, 100, 10);

    // Ratio weights are computed as (n/d). Fill has weight 1.0.
    // So Ratio(1,4) vs Fill is 0.25 vs 1.0 => 20% vs 80%.
    let flex = Flex::horizontal().constraints([Constraint::Ratio(1, 4), Constraint::Fill]);
    let rects = flex.split(area);

    assert_eq!(
        rects[0].width, 20,
        "Ratio(1, 4) vs Fill should be 20/80, got {}",
        rects[0].width
    );
    assert_eq!(
        rects[1].width, 80,
        "Ratio(1, 4) vs Fill should be 20/80, got {}",
        rects[1].width
    );
}

#[test]
fn ratio_pair_behaves_like_fraction_of_total() {
    let area = Rect::new(0, 0, 100, 10);

    // When all grow items are Ratio constraints, their weights sum to 1.0 (if chosen that way),
    // so allocations match the intended fractions of the total.
    let flex = Flex::horizontal().constraints([Constraint::Ratio(1, 4), Constraint::Ratio(3, 4)]);
    let rects = flex.split(area);

    assert_eq!(rects[0].width, 25);
    assert_eq!(rects[1].width, 75);
}
