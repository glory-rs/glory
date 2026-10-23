use glory::prelude::*;

pub const CSS: &str = ".size-box{width:200px;height:100px;max-width:80px;max-height:40px}";

pub fn stage(stage: u8) -> Option<Style> {
    let preferred = Style::new()
        .width(Length::Px(200.0))
        .height(Length::Px(100.0));
    match stage {
        0 => Some(preferred.max_width(Limit::None).max_height(Limit::None)),
        1 => Some(
            preferred
                .max_width(Limit::Px(60.0))
                .max_height(Limit::Px(30.0)),
        ),
        2 => Some(
            preferred
                .max_width(Limit::Percent(25.0))
                .max_height(Limit::Percent(20.0)),
        ),
        3 => Some(
            preferred
                .min_width(Length::Px(140.0))
                .min_height(Length::Px(70.0))
                .max_width(Limit::Px(60.0))
                .max_height(Limit::Px(30.0)),
        ),
        4 => Some(preferred),
        _ => None,
    }
}
pub fn ui() -> impl Widget {
    view()
        .class("size-frame")
        .style(
            Style::new()
                .width(Length::Px(400.0))
                .height(Length::Px(300.0)),
        )
        .fill(
            view()
                .class("size-box")
                .style(stage(0).unwrap())
                .fill(input().class("size-input").value("initial🙂")),
        )
}
