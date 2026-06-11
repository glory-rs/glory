use glory::reflow::{Bond, Cage};
use glory::web::events;
use glory::web::helpers::{event_target_checked, event_target_value};
use glory::web::widgets::*;
use glory::{Scope, Widget};

pub fn main() {
    glory::launch(FormsShowcase::new());
}

#[derive(Debug)]
struct FormsShowcase {
    name: Cage<String>,
    subscribe: Cage<bool>,
    channel: Cage<String>,
    plan: Cage<String>,
    notes: Cage<String>,
    file_value: Cage<String>,
    locked: Cage<bool>,
}

impl FormsShowcase {
    fn new() -> Self {
        Self {
            name: Cage::new("Ada".to_owned()),
            subscribe: Cage::new(true),
            channel: Cage::new("email".to_owned()),
            plan: Cage::new("pro".to_owned()),
            notes: Cage::new("Controlled textarea".to_owned()),
            file_value: Cage::new("none".to_owned()),
            locked: Cage::new(false),
        }
    }
}

impl Widget for FormsShowcase {
    fn build(&mut self, ctx: &mut Scope) {
        let scope = glory::web::scoped_css(
            r#"
            :scope {
                font-family: system-ui, sans-serif;
                max-width: 48rem;
                margin: 2rem auto;
                display: grid;
                gap: 1rem;
            }

            fieldset {
                border: 1px solid #ccc;
                display: grid;
                gap: 0.75rem;
            }

            label {
                display: grid;
                gap: 0.25rem;
            }

            .row {
                display: flex;
                gap: 1rem;
                align-items: center;
            }
            "#,
        );

        style().text(scope.css().to_owned()).show_in(ctx);

        let name = self.name;
        let update_name = move |event| {
            name.revise(|mut value| *value = event_target_value(&event));
        };

        let subscribe = self.subscribe;
        let update_subscribe = move |event| {
            subscribe.revise(|mut value| *value = event_target_checked(&event));
        };

        let channel = self.channel;
        let update_channel = move |event| {
            channel.revise(|mut value| *value = event_target_value(&event));
        };

        let plan_free = self.plan;
        let choose_free = move |_| plan_free.revise(|mut value| *value = "free".to_owned());
        let plan_pro = self.plan;
        let choose_pro = move |_| plan_pro.revise(|mut value| *value = "pro".to_owned());

        let notes = self.notes;
        let update_notes = move |event| {
            notes.revise(|mut value| *value = event_target_value(&event));
        };

        let file_value = self.file_value;
        let update_file = move |event| {
            let value = event_target_value(&event);
            file_value.revise(|mut current| *current = if value.is_empty() { "none".to_owned() } else { value });
        };

        let locked = self.locked;
        let toggle_lock = move |_| {
            locked.revise(|mut value| *value = !*value);
        };

        let summary = Bond::new({
            let name = self.name;
            let subscribe = self.subscribe;
            let channel = self.channel;
            let plan = self.plan;
            let notes = self.notes;
            let file_value = self.file_value;
            let locked = self.locked;
            move || {
                format!(
                    "name={} subscribe={} channel={} plan={} notes={} file={} locked={}",
                    name.get(),
                    subscribe.get(),
                    channel.get(),
                    plan.get(),
                    notes.get(),
                    file_value.get(),
                    locked.get(),
                )
            }
        });

        section()
            .class(scope)
            .fill(h1().text("Forms showcase"))
            .fill(
                form()
                    .attr("novalidate", true)
                    .fill(
                        fieldset()
                            .fill(legend().text("Controlled inputs"))
                            .fill(
                                label()
                                    .text("Name")
                                    .fill(input().attr("name", "name").prop("value", self.name).on(events::input, update_name)),
                            )
                            .fill(
                                label().class("row").fill(
                                    input()
                                        .attr("type", "checkbox")
                                        .attr("checked", self.subscribe)
                                        .on(events::change, update_subscribe),
                                )
                                .fill(span().text("Subscribe")),
                            )
                            .fill(
                                label().text("Preferred channel").fill(
                                    select()
                                        .attr("name", "channel")
                                        .prop("value", self.channel)
                                        .on(events::change, update_channel)
                                        .fill(option().attr("value", "email").text("Email"))
                                        .fill(option().attr("value", "sms").text("SMS"))
                                        .fill(option().attr("value", "push").text("Push")),
                                ),
                            ),
                    )
                    .fill(
                        fieldset()
                            .fill(legend().text("Radio, file, and state"))
                            .fill(
                                label().class("row").fill(
                                    input()
                                        .attr("type", "radio")
                                        .attr("name", "plan")
                                        .attr("value", "free")
                                        .attr("checked", Bond::new({
                                            let plan = self.plan;
                                            move || *plan.get() == "free"
                                        }))
                                        .on(events::change, choose_free),
                                )
                                .fill(span().text("Free")),
                            )
                            .fill(
                                label().class("row").fill(
                                    input()
                                        .attr("type", "radio")
                                        .attr("name", "plan")
                                        .attr("value", "pro")
                                        .attr("checked", Bond::new({
                                            let plan = self.plan;
                                            move || *plan.get() == "pro"
                                        }))
                                        .on(events::change, choose_pro),
                                )
                                .fill(span().text("Pro")),
                            )
                            .fill(label().text("File").fill(input().attr("type", "file").on(events::change, update_file)))
                            .fill(button().attr("type", "button").on(events::click, toggle_lock).text("Toggle lock")),
                    )
                    .fill(
                        fieldset()
                            .fill(legend().text("Controlled and uncontrolled text"))
                            .fill(
                                label().text("Notes").fill(
                                    textarea()
                                        .prop("value", self.notes)
                                        .attr("readonly", self.locked)
                                        .on(events::input, update_notes),
                                ),
                            )
                            .fill(label().text("Nickname").fill(input().attr("name", "nickname").attr("placeholder", "Uncontrolled"))),
                    ),
            )
            .fill(output().attr("aria-live", "polite").text(summary))
            .show_in(ctx);
    }
}
