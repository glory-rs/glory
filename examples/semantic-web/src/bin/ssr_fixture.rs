fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args().nth(1).ok_or("pass the build directory")?;
    let snapshot = glory::ssr::snapshot_with(|scope| glory_semantic_web::hydration::ui(scope).0)?;
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Glory SSR adoption acceptance</title><style data-gly-base=\"v1\">{}</style><style>.h-list>[role=list]{{width:600px;border:1px solid #888}}body{{font-family:sans-serif}}</style></head><body><div id=\"ssr\">{}</div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        snapshot.html
    );
    std::fs::write(std::path::Path::new(&directory).join("ssr.html"), page)?;
    let input_snapshot =
        glory::ssr::snapshot_with(|scope| glory_semantic_web::input_form::ui(scope).0)?;
    let input_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Input contract</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"input-ssr\">{}</div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        input_snapshot.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("inputs.html"),
        input_page,
    )?;
    let semantics = glory::ssr::snapshot(glory_semantic_web::semantics_form::ui())?;
    let semantics_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Semantic metadata</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"semantics-ssr\">{}</div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        semantics.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("semantics.html"),
        semantics_page,
    )?;
    let sizes = glory::ssr::snapshot(glory_semantic_web::size_form::ui())?;
    let size_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Size constraints</title><style data-gly-base=\"v1\">{}</style><style>{}</style></head><body><div id=\"size-ssr\">{}</div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        glory_semantic_web::size_form::CSS,
        sizes.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("sizes.html"),
        size_page,
    )?;
    let flex = glory::ssr::snapshot(glory_semantic_web::flex_form::ui(0))?;
    let flex_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Flex layout</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"flex-ssr\">{}</div><div id=\"app\"></div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        flex.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("flex.html"),
        flex_page,
    )?;
    let fields = glory::ssr::snapshot_with(|scope| glory_semantic_web::fields_form::ui(scope).0)?;
    let fields_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Field dependencies</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"fields-ssr\">{}</div><div id=\"app\"></div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        fields.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("fields.html"),
        fields_page,
    )?;
    let entries = glory::ssr::snapshot_with(|scope| glory_semantic_web::entries_form::ui(scope).0)?;
    let entries_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Keyed collection paths</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"entries-ssr\">{}</div><div id=\"app\"></div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        entries.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("entries.html"),
        entries_page,
    )?;
    let css = glory::html::sheet(
        &glory_semantic_web::css_form::sheet(),
        glory::style::Profile::HTML,
    )?;
    let styled = glory::ssr::snapshot_with(|scope| glory_semantic_web::css_form::ui(scope).0)?;
    let styled_page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Portable CSS</title><style data-gly-base=\"v1\">{}</style><style data-gly-sheet=\"v1\">{css}</style></head><body><div id=\"css-ssr\">{}</div><div id=\"app\"></div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        styled.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("css.html"),
        styled_page,
    )?;
    let mut seeds = glory::resources::Snapshot::default();
    seeds.seed(
        "profile",
        "profile/1",
        &7_u64,
        Ok::<_, &String>(&glory_semantic_web::resource_hydration::VALUE.to_owned()),
    )?;
    seeds.seed(
        "problem",
        "problem/1",
        &0_u64,
        Err::<&String, _>(&"server error".to_owned()),
    )?;
    let resources = glory::resources::Resources::restore(seeds)?;
    let snapshot = glory::ssr::snapshot_resources_with(&resources, |_| {
        glory_semantic_web::resource_hydration::widget(resources.clone(), Default::default())
    })?;
    let data = glory::ssr::resource_script("resource-data", &snapshot.data, Some("fixture"))?;
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; script-src 'self' 'wasm-unsafe-eval' 'nonce-fixture'; style-src 'nonce-fixture'; connect-src 'self'\"><title>Resource snapshot acceptance</title><style nonce=\"fixture\" data-gly-base=\"v1\">{}</style></head><body><div id=\"resource-ssr\">{}</div>{data}<script type=\"module\" src=\"./resource-bootstrap.js\"></script></body></html>",
        glory::html::BASE_CSS,
        snapshot.view.html
    );
    std::fs::write(
        std::path::Path::new(&directory).join("resource-ssr.html"),
        page,
    )?;
    std::fs::write(
        std::path::Path::new(&directory).join("resource-bootstrap.js"),
        "import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;",
    )?;
    let owners = glory::ssr::snapshot_with(|scope| glory_semantic_web::owner_form::ui(scope).0)?;
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Owner lifetime</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"owner-ssr\">{}</div><div id=\"app\"></div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        owners.html
    );
    std::fs::write(std::path::Path::new(&directory).join("owners.html"), page)?;
    let queries = glory::ssr::snapshot_with(|scope| glory_semantic_web::query_form::ui(scope).0)?;
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Scoped query lifetime</title><style data-gly-base=\"v1\">{}</style></head><body><div id=\"query-ssr\">{}</div><div id=\"app\"></div><script type=\"module\">import init,* as demo from './glory_semantic_web.js';await init();window.demo=demo;window.ready=true;</script></body></html>",
        glory::html::BASE_CSS,
        queries.html
    );
    std::fs::write(std::path::Path::new(&directory).join("queries.html"), page)?;
    Ok(())
}
