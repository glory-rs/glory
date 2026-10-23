use glory::html::{HtmlTextExt, TextTag};
use glory::prelude::*;

#[test]
fn facade_connects_typed_html_options_to_the_shared_mapper() {
    let output = glory::ssr::render(
        text()
            .content("body")
            .html_options(|html| html.tag(TextTag::P)),
    )
    .unwrap();
    assert!(output.starts_with("<p "));
    assert!(output.ends_with(">body</p>"));
    assert!(
        glory::ssr::render(
            text()
                .kind(TextKind::Heading(1))
                .html_options(|html| html.tag(TextTag::P))
        )
        .is_err()
    );
}

#[test]
fn html_text_override_cannot_bypass_button_content_rules() {
    assert!(
        glory::ssr::render(
            button().fill(
                text()
                    .content("body")
                    .html_options(|html| html.tag(TextTag::P))
            )
        )
        .is_err()
    );
    assert!(
        glory::ssr::render(
            button().fill(
                group().fill(
                    text()
                        .content("body")
                        .html_options(|html| html.tag(TextTag::P))
                )
            )
        )
        .is_err()
    );
    let output = glory::ssr::render(
        button().fill(
            group().fill(
                text()
                    .content("body")
                    .html_options(|html| html.tag(TextTag::Span)),
            ),
        ),
    )
    .unwrap();
    assert!(output.contains(">body</span>"));
}
