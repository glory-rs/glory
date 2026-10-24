use glory_style::{
    AspectRatio, Display, GridPlacement, GridTrack, GridTracks, MAX_ASPECT_RATIO, MAX_GRID_TRACKS,
    MIN_ASPECT_RATIO, Profile, Property, Style, Support, Unsupported, Value,
};
fn tracks(rows: bool, value: GridTracks) -> Style {
    if rows {
        Style::new().grid_rows(value)
    } else {
        Style::new().grid_columns(value)
    }
}
fn placement(rows: bool, value: GridPlacement) -> Style {
    if rows {
        Style::new().grid_row(value)
    } else {
        Style::new().grid_column(value)
    }
}
fn unsupported(property: Property, value: Option<Value>) -> Unsupported {
    Unsupported { property, value }
}
#[test]
fn tracks_have_independent_property_list_and_each_original_unit_rejection() {
    for rows in [false, true] {
        let property = if rows {
            Property::GridRows
        } else {
            Property::GridColumns
        };
        for (track, unit) in [
            (GridTrack::Auto, Value::Auto),
            (GridTrack::Px(0.0), Value::Px),
            (GridTrack::Percent(25.0), Value::Percent),
            (GridTrack::Fr(1.5), Value::Fr),
        ] {
            let style = tracks(rows, GridTracks::Tracks(vec![track]));
            style.validate().unwrap();
            Profile::HTML.inline.check(&style).unwrap();
            assert_eq!(
                Support::ALL.without_property(property).check(&style),
                Err(unsupported(property, None))
            );
            assert_eq!(
                Support::ALL.without_value(Value::TrackList).check(&style),
                Err(unsupported(property, Some(Value::TrackList)))
            );
            assert_eq!(
                Support::ALL.without_value(unit).check(&style),
                Err(unsupported(property, Some(unit)))
            );
        }
        let reset = tracks(rows, GridTracks::None);
        assert_eq!(
            Support::ALL.without_value(Value::None).check(&reset),
            Err(unsupported(property, Some(Value::None)))
        );
        Support::ALL
            .without_value(Value::TrackList)
            .check(&reset)
            .unwrap();
    }
    const {
        assert!(MAX_GRID_TRACKS <= 64);
    }
    const {
        assert!(Property::ALL.len() <= 64 && Value::ALL.len() <= 64);
    }
    let style = Style::new().display(Display::Grid);
    assert_eq!(
        Support::ALL.without_value(Value::Grid).check(&style),
        Err(unsupported(Property::Display, Some(Value::Grid)))
    );
}
#[test]
fn track_lists_enforce_length_finiteness_and_sign_budget() {
    for rows in [false, true] {
        tracks(rows, GridTracks::Tracks(vec![GridTrack::Auto; 64]))
            .validate()
            .unwrap();
        for values in [
            vec![],
            vec![GridTrack::Auto; 65],
            vec![GridTrack::Px(-1.0)],
            vec![GridTrack::Percent(-0.1)],
            vec![GridTrack::Fr(0.0)],
            vec![GridTrack::Fr(-1.0)],
            vec![GridTrack::Px(f64::NAN)],
            vec![GridTrack::Percent(f64::INFINITY)],
            vec![GridTrack::Fr(f64::INFINITY)],
        ] {
            assert!(tracks(rows, GridTracks::Tracks(values)).validate().is_err());
        }
    }
}
#[test]
fn placement_units_and_both_axes_have_independent_rejection_and_boundaries() {
    for rows in [false, true] {
        let property = if rows {
            Property::GridRow
        } else {
            Property::GridColumn
        };
        for (value, unit) in [
            (GridPlacement::Auto, Value::Auto),
            (GridPlacement::Span(64), Value::PlacementSpan),
            (GridPlacement::At { start: 64, span: 1 }, Value::PlacementAt),
            (GridPlacement::At { start: 1, span: 64 }, Value::PlacementAt),
        ] {
            let style = placement(rows, value);
            style.validate().unwrap();
            assert_eq!(
                Support::ALL.without_property(property).check(&style),
                Err(unsupported(property, None))
            );
            assert_eq!(
                Support::ALL.without_value(unit).check(&style),
                Err(unsupported(property, Some(unit)))
            );
        }
        for value in [
            GridPlacement::Span(0),
            GridPlacement::Span(65),
            GridPlacement::At { start: 0, span: 1 },
            GridPlacement::At { start: 1, span: 0 },
            GridPlacement::At { start: 64, span: 2 },
            GridPlacement::At {
                start: u16::MAX,
                span: u16::MAX,
            },
        ] {
            assert!(placement(rows, value).validate().is_err());
        }
    }
}
#[test]
fn ratio_property_units_numeric_domain_and_auto_reset_are_independent() {
    for (ratio, unit) in [
        (AspectRatio::Auto, Value::Auto),
        (AspectRatio::Ratio(16.0 / 9.0), Value::Ratio),
        (AspectRatio::Ratio(MIN_ASPECT_RATIO), Value::Ratio),
        (AspectRatio::Ratio(MAX_ASPECT_RATIO), Value::Ratio),
    ] {
        let style = Style::new().aspect_ratio(ratio);
        style.validate().unwrap();
        assert_eq!(
            Support::ALL
                .without_property(Property::AspectRatio)
                .check(&style),
            Err(unsupported(Property::AspectRatio, None))
        );
        assert_eq!(
            Support::ALL.without_value(unit).check(&style),
            Err(unsupported(Property::AspectRatio, Some(unit)))
        );
    }
    for ratio in [
        0.0,
        -1.0,
        MIN_ASPECT_RATIO / 2.0,
        MAX_ASPECT_RATIO * 2.0,
        f64::NAN,
        f64::INFINITY,
    ] {
        assert!(
            Style::new()
                .aspect_ratio(AspectRatio::Ratio(ratio))
                .validate()
                .is_err()
        );
    }
}
#[cfg(feature = "css")]
#[test]
fn finite_grid_css_parses_ordered_tracks_placements_and_ratios() {
    let sheet=glory_style::Sheet::parse(".grid{display:grid;grid-template-columns:0 25% 2fr auto;grid-template-rows:none;grid-column:2 / span 3;grid-row:span 2;aspect-ratio:16 / 9}").unwrap();
    sheet.check(Profile::HTML).unwrap();
    let values = sheet.rules()[0].declarations();
    assert_eq!(
        values[1].style().grid_columns,
        Some(GridTracks::Tracks(vec![
            GridTrack::Px(0.0),
            GridTrack::Percent(25.0),
            GridTrack::Fr(2.0),
            GridTrack::Auto
        ]))
    );
    assert_eq!(values[2].style().grid_rows, Some(GridTracks::None));
    assert_eq!(
        values[3].style().grid_column,
        Some(GridPlacement::At { start: 2, span: 3 })
    );
    assert_eq!(values[4].style().grid_row, Some(GridPlacement::Span(2)));
    assert_eq!(
        values[5].style().aspect_ratio,
        Some(AspectRatio::Ratio(16.0 / 9.0))
    );
}

#[cfg(feature = "css")]
#[test]
fn css_ratio_domain_endpoints_preserve_declared_numeric_precision() {
    for (value, expected) in [
        ("0.000001", MIN_ASPECT_RATIO),
        ("1000000", MAX_ASPECT_RATIO),
        ("/*a*/0.000001/*b*/ / /*c*/ 1", MIN_ASPECT_RATIO),
        ("+1e-6", MIN_ASPECT_RATIO),
    ] {
        let sheet = glory_style::Sheet::parse(&format!(".x{{aspect-ratio:{value}}}")).unwrap();
        assert_eq!(
            sheet.rules()[0].declarations()[0].style().aspect_ratio,
            Some(AspectRatio::Ratio(expected))
        );
    }
}
#[cfg(feature = "css")]
#[test]
fn grid_css_rejects_unbounded_functions_names_negative_lines_and_outside_ratio_domain() {
    for value in [
        "repeat(2,1fr)",
        "minmax(0,1fr)",
        "subgrid",
        "[named] 1fr",
        "fit-content(10px)",
        "1em",
        "-1px",
        "0fr",
        "none auto",
        "auto,auto",
    ] {
        assert!(
            glory_style::Sheet::parse(&format!(".x{{grid-template-columns:{value}}}")).is_err(),
            "{value}"
        );
    }
    let huge = vec!["1fr"; 65].join(" ");
    assert!(glory_style::Sheet::parse(&format!(".x{{grid-template-rows:{huge}}}")).is_err());
    for value in [
        "-1 / span 1",
        "0 / span 1",
        "64 / span 2",
        "span 0",
        "span 65",
        "named",
        "2",
        "1 / 3",
        "1 / span 2 extra",
    ] {
        assert!(
            glory_style::Sheet::parse(&format!(".x{{grid-row:{value}}}")).is_err(),
            "{value}"
        );
    }
    for value in [
        "0",
        "-1",
        "0 / 1",
        "1 / 0",
        "-1 / -1",
        "1 / -1",
        "auto 2",
        "0.0000001",
        "10000000",
    ] {
        assert!(
            glory_style::Sheet::parse(&format!(".x{{aspect-ratio:{value}}}")).is_err(),
            "{value}"
        );
    }
}
#[cfg(feature = "css")]
#[test]
fn all_original_grid_and_ratio_declarations_are_checked_before_overrides() {
    for (css, property, unit) in [
        (
            "grid-template-columns:1fr;grid-template-columns:10px",
            Property::GridColumns,
            Value::Fr,
        ),
        (
            "grid-template-rows:10%;grid-template-rows:none",
            Property::GridRows,
            Value::Percent,
        ),
        (
            "grid-column:span 2;grid-column:auto",
            Property::GridColumn,
            Value::PlacementSpan,
        ),
        (
            "grid-row:1 / span 2;grid-row:auto",
            Property::GridRow,
            Value::PlacementAt,
        ),
        (
            "aspect-ratio:2;aspect-ratio:auto",
            Property::AspectRatio,
            Value::Ratio,
        ),
    ] {
        let sheet = glory_style::Sheet::parse(&format!(".x{{{css}}}")).unwrap();
        let profile = Profile {
            inline: Support::ALL.without_value(unit),
            ..Profile::HTML
        };
        let error = sheet.check(profile).unwrap_err();
        assert!(
            format!("{error:?}").contains(&format!("{property:?}")),
            "{error:?}"
        );
        assert!(
            sheet
                .check(Profile {
                    inline: Support::ALL.without_property(property),
                    ..Profile::HTML
                })
                .is_err()
        );
    }
}
