use crate::config::{GloryConfig, ReloadWebSocketProtocol};
use crate::web::widgets::{DEPOT_BODY_META_KEY, DEPOT_HEAD_MIXIN_KEY, DEPOT_HTML_META_KEY};
use crate::{Node, Truck};

fn auto_reload(nonce_str: &str, options: &GloryConfig) -> String {
    let reload_port = match options.reload_external_port {
        Some(val) => val,
        None => options.reload_port,
    };
    let protocol = match options.reload_protocol {
        ReloadWebSocketProtocol::Ws => "'ws://'",
        ReloadWebSocketProtocol::Wss => "'wss://'",
    };
    if std::env::var("GLORY_WATCH").is_ok() {
        format!(
            r#"<script crossorigin=""{nonce_str}>(function () {{
    {}
    let host = window.location.hostname;
    const OVERLAY_ID = '__glory_build_error_overlay__';
    const clearOverlay = () => {{
        const existing = document.getElementById(OVERLAY_ID);
        if (existing) existing.remove();
    }};
    const showBuildError = (message) => {{
        clearOverlay();
        const overlay = document.createElement('div');
        overlay.id = OVERLAY_ID;
        overlay.style.cssText = 'position:fixed;inset:0;z-index:2147483647;background:rgba(20,20,20,0.92);color:#ff8585;font:13px/1.5 ui-monospace,SFMono-Regular,Menlo,monospace;padding:0;overflow:auto;';
        const bar = document.createElement('div');
        bar.style.cssText = 'display:flex;justify-content:space-between;align-items:center;padding:12px 16px;background:#1b1b1b;color:#fff;font-weight:600;position:sticky;top:0;';
        const title = document.createElement('span');
        title.textContent = 'Build failed';
        const close = document.createElement('button');
        close.textContent = '×';
        close.setAttribute('aria-label', 'Dismiss build error');
        close.style.cssText = 'background:none;border:none;color:#fff;font-size:22px;line-height:1;cursor:pointer;';
        close.onclick = () => overlay.remove();
        bar.appendChild(title);
        bar.appendChild(close);
        const pre = document.createElement('pre');
        pre.style.cssText = 'margin:0;padding:16px;white-space:pre-wrap;word-break:break-word;';
        // textContent, never innerHTML: compiler output is not markup.
        pre.textContent = message || 'Build failed. See the terminal running `glory serve`.';
        overlay.appendChild(bar);
        overlay.appendChild(pre);
        document.body.appendChild(overlay);
    }};
    let reconnectMs = 500;
    const connect = () => {{
    let ws = new WebSocket({protocol} + host + ':{reload_port}/live_reload');
    ws.onopen = () => {{ reconnectMs = 500; }};
    ws.onmessage = (ev) => {{
        let msg = JSON.parse(ev.data);
        if (msg.type === 'full') window.location.reload();
        if (msg.type === 'style') {{
            clearOverlay();
            let found = false;
            document.querySelectorAll("link").forEach((link) => {{
                const href = link.getAttribute('href');
                if (href && href.includes(msg.css_path)) {{
                    let newHref = '/' + msg.css_path + '?version=' + Date.now();
                    link.setAttribute('href', newHref);
                    found = true;
                }}
            }});
            if (!found) console.warn(`CSS hot-reload: Could not find a <link href=/\"${{msg.css_path}}\"> element`);
        }};
        if (msg.type === 'build_error') showBuildError(msg.message);
        if(msg.type === 'functions') {{
            window.dispatchEvent(new CustomEvent('glory:function-reload', {{ detail: JSON.parse(msg.payload) }}));
        }}
    }};
    ws.onclose = () => {{
        // The dev server restarts on its own (a server rebuild, a crash); back
        // off and reconnect instead of stranding the page.
        console.warn(`Live-reload disconnected, retrying in ${{reconnectMs}}ms.`);
        setTimeout(connect, reconnectMs);
        reconnectMs = Math.min(reconnectMs * 2, 10000);
    }};
    }};
    connect();
}})()
</script>"#,
            glory_hot_reload::HOT_RELOAD_JS
        )
    } else {
        "".into()
    }
}

fn nonce_attr(config: &GloryConfig) -> String {
    config
        .csp_nonce
        .as_deref()
        .map(|nonce| format!(r#" nonce="{}""#, escape_html_attr(nonce)))
        .unwrap_or_default()
}

fn escape_html_attr(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STREAM_HYDRATE_JS: &str = r#"(function () {
    if (window.__gloryStreamHydrate) return;

    function cssEscape(value) {
        if (window.CSS && CSS.escape) return CSS.escape(value);
        return String(value).replace(/["\\]/g, "\\$&");
    }

    function markerSelector(id) {
        return 'template[data-glory-placeholder="' + cssEscape(id) + '"]';
    }

    function patchSelector(id) {
        return 'template[data-glory-placeholder-patch="' + cssEscape(id) + '"]';
    }

    const api = {
        patchFromTemplate(id) {
            const patch = document.querySelector(patchSelector(id));
            const marker = document.querySelector(markerSelector(id));
            if (!patch || !marker) return false;
            marker.replaceWith(patch.content.cloneNode(true));
            patch.remove();
            return true;
        },
        flush() {
            document.querySelectorAll("template[data-glory-placeholder-patch]").forEach((patch) => {
                const id = patch.getAttribute("data-glory-placeholder-patch");
                if (id) this.patchFromTemplate(id);
            });
        },
    };

    window.__gloryStreamHydrate = api;

    function observe() {
        api.flush();
        if (!("MutationObserver" in window)) return;
        new MutationObserver(() => api.flush()).observe(document.documentElement, {
            childList: true,
            subtree: true,
        });
    }

    if (document.documentElement) observe();
    else document.addEventListener("DOMContentLoaded", observe, { once: true });
})()"#;

#[cfg(feature = "web-ssr")]
#[tracing::instrument(level = "trace", fields(error), skip_all)]
pub fn html_parts_separated(
    config: &GloryConfig,
    truck: &Truck,
    document: &crate::renderer::ssr_dom::SsrDocument,
) -> (String, String, &'static str) {
    let pkg_path = &config.site_pkg_dir;
    let output_name = &config.output_name;

    // wasm-bindgen emits the wasm artifact with an `_bg` suffix when the
    // CLI has not pinned GLORY_OUTPUT_NAME at compile time.
    let mut wasm_output_name = output_name.clone();
    if std::option_env!("GLORY_OUTPUT_NAME").is_none() {
        wasm_output_name.push_str("_bg");
    }

    let nonce = nonce_attr(config);
    let glory_auto_reload = auto_reload(&nonce, config);

    // The truck stores command-stream node handles; their rendered content
    // lives in the replayed document.
    let html_open = truck
        .get::<Node>(DEPOT_HTML_META_KEY)
        .ok()
        .and_then(|node| document.html_tag(node.id()))
        .map(|(open, _)| open)
        .unwrap_or_else(|| "<html>".into());
    let body_open = truck
        .get::<Node>(DEPOT_BODY_META_KEY)
        .ok()
        .and_then(|node| document.html_tag(node.id()))
        .map(|(open, _)| open)
        .unwrap_or_else(|| "<body>".into());

    let head_mixin = truck
        .get::<Node>(DEPOT_HEAD_MIXIN_KEY)
        .ok()
        .map(|node| document.inner_html(node.id()))
        .unwrap_or_default();

    (
        format!(
            r#"<!doctype html>
{html_open}
    <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        {head_mixin}
        <link rel="modulepreload" href="/{pkg_path}/{output_name}.js"{nonce}>
        <link rel="preload" href="/{pkg_path}/{wasm_output_name}.wasm" as="fetch" type="application/wasm" crossorigin=""{nonce}>
        <script{nonce}>{STREAM_HYDRATE_JS}</script>
        <script type="module"{nonce}>
        function idle(c) {{
            if ("requestIdleCallback" in window) {{window.requestIdleCallback(c);}} else {{c();}}
        }}
        idle(() => {{
            import('/{pkg_path}/{output_name}.js').then(mod => {{mod.default('/{pkg_path}/{wasm_output_name}.wasm')}});
        }});
        </script>{glory_auto_reload}"#
        ),
        format!("\n    </head>\n    {body_open}"),
        "\n    </body>\n</html>",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssr_head_installs_stream_hydrate_runtime() {
        let config = GloryConfig::default();
        let truck = Truck::new();
        let document = crate::renderer::ssr_dom::SsrDocument::new();
        let (head, _, _) = html_parts_separated(&config, &truck, &document);

        assert!(head.contains("window.__gloryStreamHydrate"));
        assert!(head.contains("patchFromTemplate"));
        assert!(head.contains("template[data-glory-placeholder-patch]"));
    }

    #[test]
    fn ssr_head_applies_escaped_csp_nonce() {
        let config = GloryConfig {
            csp_nonce: Some(r#""<&>""#.to_string()),
            ..GloryConfig::default()
        };
        let truck = Truck::new();
        let document = crate::renderer::ssr_dom::SsrDocument::new();
        let (head, _, _) = html_parts_separated(&config, &truck, &document);

        assert!(head.contains(r#"nonce="&quot;&lt;&amp;&gt;&quot;""#));
    }
}
