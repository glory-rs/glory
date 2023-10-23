use crate::config::{GloryConfig, ReloadWSProtocol};
use crate::web::widgets::{DEPOT_BODY_META_KEY, DEPOT_HEAD_MIXIN_KEY, DEPOT_HTML_META_KEY};
use crate::{Node, Truck};

fn auto_reload(nonce_str: &str, options: &GloryConfig) -> String {
    let reload_port = match options.reload_external_port {
        Some(val) => val,
        None => options.reload_port,
    };
    let protocol = match options.reload_ws_protocol {
        ReloadWSProtocol::WS => "'ws://'",
        ReloadWSProtocol::WSS => "'wss://'",
    };
    if std::env::var("GLORY_WATCH").is_ok() {
        format!(
            r#"<script crossorigin=""{nonce_str}>(function () {{
    {}
    let host = window.location.hostname;
    let ws = new WebSocket({protocol} + host + ':{reload_port}/live_reload');
    ws.onmessage = (ev) => {{
        let msg = JSON.parse(ev.data);
        if (msg.all) window.location.reload();
        if (msg.css) {{
            let found = false;
            document.querySelectorAll("link").forEach((link) => {{
                if (link.getAttribute('href').includes(msg.css)) {{
                    let newHref = '/' + msg.css + '?version=' + new Date().getMilliseconds();
                    link.setAttribute('href', newHref);
                    found = true;
                }}
            }});
            if (!found) console.warn(`CSS hot-reload: Could not find a <link href=/\"${{msg.css}}\"> element`);
        }};
        if(msg.view) {{
            patch(msg.view);
        }}
    }};
    ws.onclose = () => console.warn('Live-reload stopped. Manual reload necessary.');
}})()
</script>"#,
            glory_hot_reload::HOT_RELOAD_JS
        )
    } else {
        "".into()
    }
}

#[cfg(feature = "web-ssr")]
#[tracing::instrument(level = "trace", fields(error), skip_all)]
pub fn html_parts_separated(config: &GloryConfig, truck: &Truck) -> (String, String, &'static str) {
    let pkg_path = &config.site_pkg_dir;
    let output_name = &config.output_name;

    // Because wasm-pack adds _bg to the end of the WASM filename, and we want to maintain compatibility with it's default options
    // we add _bg to the wasm files if glory-cli doesn't set the env var GLORY_OUTPUT_NAME at compile time
    // Otherwise we need to add _bg because wasm_pack always does.
    let mut wasm_output_name = output_name.clone();
    if std::option_env!("GLORY_OUTPUT_NAME").is_none() {
        wasm_output_name.push_str("_bg");
    }

    let nonce = "";
    let glory_auto_reload = auto_reload(&nonce, config);

    let html_open = if let Ok(node) = truck.get::<Node>(DEPOT_HTML_META_KEY) {
        node.html_tag().0
    } else {
        "<html>".into()
    };
    let body_open = if let Ok(node) = truck.get::<Node>(DEPOT_BODY_META_KEY) {
        node.html_tag().0
    } else {
        "<body>".into()
    };

    let head_mixin = if let Ok(node) = truck.get::<Node>(DEPOT_HEAD_MIXIN_KEY) {
        node.inner_html()
    } else {
        "".into()
    };

    (
        format!(
            r#"<!DOCTYPE html>
{html_open}
    <head>
        <meta charset="utf-8"/>
        <meta name="viewport" content="width=device-width, initial-scale=1"/>
        <link rel="modulepreload" href="/{pkg_path}/{output_name}.js"{nonce}>
        <link rel="preload" href="/{pkg_path}/{wasm_output_name}.wasm" as="fetch" type="application/wasm" crossorigin=""{nonce}>
        <script type="module"{nonce}>
        function idle(c) {{
            if ("requestIdleCallback" in window) {{window.requestIdleCallback(c);}} else {{c();}}
        }}
        idle(() => {{
            import('/{pkg_path}/{output_name}.js').then(mod => {{mod.default('/{pkg_path}/{wasm_output_name}.wasm')}});
        }});
        </script>{glory_auto_reload}{head_mixin}"#
        ),
        format!("\n    </head>\n    {body_open}"),
        "\n    </body>\n</html>",
    )
}
