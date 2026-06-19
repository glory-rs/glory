use glory_core::reflow::Cage;
use glory_core::web::events;
use glory_core::web::helpers::event_target_value;
use glory_core::web::widgets::*;
use glory_core::{Scope, Widget};
use glory_liveview::{LIVEVIEW_CLIENT_JS, LIVEVIEW_DEFAULT_PATH, salvo_mount};
use salvo::prelude::*;

#[derive(Debug)]
struct Counter {
    value: Cage<i64>,
}

impl Counter {
    fn new() -> Self {
        Self { value: Cage::new(0) }
    }
}

impl Widget for Counter {
    fn build(&mut self, ctx: &mut Scope) {
        let increase = {
            let value = self.value;
            move |_| value.revise(|mut value| *value += 1)
        };
        let decrease = {
            let value = self.value;
            move |_| value.revise(|mut value| *value -= 1)
        };
        let clear = {
            let value = self.value;
            move |_| value.revise(|mut value| *value = 0)
        };
        let set_from_input = {
            let value = self.value;
            move |ev| {
                let parsed = event_target_value(&ev).parse::<i64>().unwrap_or_default();
                value.revise(|mut value| *value = parsed);
            }
        };

        div()
            .attr("style", "font-family: system-ui, sans-serif; padding: 2rem; display: flex; gap: .5rem; align-items: center;")
            .fill(button().text("-").on(events::click, decrease))
            .fill(
                span()
                    .attr("style", "min-width: 4rem; text-align: center; font-size: 1.5rem;")
                    .text(self.value),
            )
            .fill(button().text("+").on(events::click, increase))
            .fill(button().text("clear").on(events::click, clear))
            .fill(input().attr("placeholder", "set value").on(events::input, set_from_input))
            .show_in(ctx);
    }
}

#[handler]
async fn index(res: &mut Response) {
    res.render(Text::Html(index_html()));
}

fn index_html() -> String {
    format!(
        r#"<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Glory LiveView Salvo</title>
</head>
<body>
  <script>{interpreter}</script>
  <script>{client}</script>
  <script>window.__gloryLiveViewConnect({path:?});</script>
</body>
</html>"#,
        interpreter = glory_desktop::WRY_INTERPRETER_JS,
        client = LIVEVIEW_CLIENT_JS,
        path = LIVEVIEW_DEFAULT_PATH,
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();

    let router = Router::new()
        .get(index)
        .push(salvo_mount::router(Counter::new));
    let acceptor = TcpListener::new("127.0.0.1:8080").bind().await;
    println!("LiveView Salvo example: http://127.0.0.1:8080/");
    Server::new(acceptor).serve(router).await;
}
