use glory::prelude::*;
use glory::render::Event;
pub const CSS: &str = ".sheet-pane{display:flex;flex-direction:column;width:320px;padding:8px;gap:4px;color:#123456;font-size:16px;line-height:20px}.sheet-pane>.sheet-box{width:80px;height:24px;background-color:#112233}.sheet-box.hot{width:120px}.sheet-box{width:90px}.sheet-box:hover{height:28px}.sheet-check{display:block;width:70px;height:20px}.sheet-check:enabled{width:75px}.sheet-check:checked{height:40px}.sheet-check:disabled{width:100px}.sheet-check:focus{height:60px}.sheet-input{width:120px;height:30px}.sheet-input:focus{width:150px}";
pub struct State {
    pub checked: Cage<bool>,
    pub enabled: Cage<bool>,
    pub hot: Cage<bool>,
    pub typography: Cage<Style>,
}
pub fn ui(scope: &Scope) -> (impl Widget + use<>, State) {
    let checked = scope.cage(false);
    let enabled = scope.cage(true);
    let hot = scope.cage(false);
    let typography = scope.cage(Style::new());
    let widget = view().class("sheet-pane").fill((
        group().fill(view().class(hot.map(|v| {
            if *v {
                "sheet-box hot".into()
            } else {
                "sheet-box".into()
            }
        }))),
        check()
            .class("sheet-check")
            .label("Choice")
            .checked(checked)
            .enabled(enabled)
            .on(checked_changed, move |event| {
                if let Event::CheckedChanged(value) = event {
                    checked.revise(|v| *v = value);
                }
            }),
        input().class("sheet-input").value("initial🙂"),
        text().class("sheet-text").content("inherited"),
        view().class("sheet-type").style(typography).fill((
            text()
                .kind(TextKind::Paragraph)
                .class("sheet-words")
                .content("alpha beta gamma delta epsilon zeta eta theta"),
            input().class("sheet-editor").value("font draft🙂"),
        )),
    ));
    (
        widget,
        State {
            checked,
            enabled,
            hot,
            typography,
        },
    )
}
pub fn sheet() -> glory::style::Sheet {
    glory::style::Sheet::parse(&format!("{CSS}.sheet-type{{width:160px;font-family:'Glory deliberately missing face',monospace;font-weight:450.5;white-space:nowrap;border-width:2px;border-style:solid;border-color:#010203;border-radius:8px;opacity:.5}}.sheet-editor{{width:120px;height:30px}}"))
        .expect("valid finite fixture stylesheet")
}
