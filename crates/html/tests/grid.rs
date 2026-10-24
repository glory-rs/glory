use glory_render::style::{AspectRatio, Display, GridPlacement, GridTrack, GridTracks, Style};
#[test]
fn grid_and_ratio_serialize_independent_longhands_with_explicit_resets() {
    let style = Style::new()
        .display(Display::Grid)
        .grid_columns(GridTracks::Tracks(vec![
            GridTrack::Px(20.0),
            GridTrack::Percent(25.0),
            GridTrack::Fr(2.0),
            GridTrack::Auto,
        ]))
        .grid_rows(GridTracks::None)
        .grid_column(GridPlacement::At { start: 2, span: 3 })
        .grid_row(GridPlacement::Span(2))
        .aspect_ratio(AspectRatio::Ratio(1.5));
    assert_eq!(
        glory_html::style(&style),
        "grid-template-columns:20px 25% 2fr auto;grid-template-rows:none;grid-column:2 / span 3;grid-row:span 2;aspect-ratio:1.5;display:grid;"
    );
    assert_eq!(
        glory_html::style(
            &Style::new()
                .grid_columns(GridTracks::None)
                .grid_row(GridPlacement::Auto)
                .grid_column(GridPlacement::Auto)
                .aspect_ratio(AspectRatio::Auto)
        ),
        "grid-template-columns:none;grid-column:auto;grid-row:auto;aspect-ratio:auto;"
    );
    assert_eq!(glory_html::style(&Style::new()), "");
}
