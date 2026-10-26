//! Small, identical-entry-point probes. Build exactly one use-* scenario at a time.
#![cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(feature = "base")]
#[allow(unused_imports)] // Each probe compiles a different subset of the builders.
use glory::prelude::*;
#[cfg(feature = "base")]
thread_local! {
    static APP: std::cell::RefCell<Option<glory::web::WebApp>> = const { std::cell::RefCell::new(None) };
}

#[cfg(feature = "base")]
fn ui(scope: &glory::Scope) -> impl glory::Widget + use<> {
    let _ = scope;
    #[cfg(feature = "minimal")]
    return group();
    #[cfg(feature = "button")]
    {
        let count = scope.cage(0_u32);
        return button()
            .label(count.map(|n| format!("Count {n}")))
            .on(activate, move |_| count.revise(|n| *n += 1));
    }
    #[cfg(feature = "sapid-button")]
    {
        let count = scope.cage(0_u32);
        return sapid::button()
            .label(count.map(|n| format!("Count {n}")))
            .on_press(move || count.revise(|n| *n += 1));
    }
    #[cfg(feature = "use-code")]
    return sapid::code_highlight()
        .source("fn main() { let answer = 42; }")
        .language(sapid::CodeLanguage::Rust);
    #[cfg(feature = "use-json")]
    return sapid::json_viewer("sample")
        .source("{\"answer\":42,\"items\":[true,false]}")
        .tree_navigation(true);
    #[cfg(feature = "use-dates")]
    {
        let date = sapid::CivilDate::parse_iso("2024-02-29").unwrap();
        return sapid::date_input("sample", sapid::DatesProvider::new(date))
            .default_value(Some(date));
    }
    #[cfg(feature = "use-files")]
    return sapid::file_button().label("Choose file");
    #[cfg(feature = "use-chat")]
    return sapid::chat::suggestion("sample").label("Explain this code");
    #[cfg(feature = "use-editor")]
    return sapid::rich_text_editor().name("Article").toolbar(false);
    #[cfg(feature = "use-charts")]
    {
        use sapid::charts::*;
        let data = ChartData {
            categories: vec![ChartCategory::new("a", "A"), ChartCategory::new("b", "B")],
            series: vec![ChartSeries::new("sample", "Sample", [Some(2.0), Some(5.0)])],
        };
        return line_chart("sample").data(data);
    }
    #[cfg(feature = "use-i18n")]
    {
        use glory::i18n::{Args, Catalog, I18n};
        let catalog =
            std::rc::Rc::new(Catalog::new("en", [("en", "hello = Hello, { $name }!")]).unwrap());
        let language = scope.cage("en".parse::<glory::i18n::Locale>().unwrap());
        let i18n = I18n::new(catalog, language);
        let message = i18n
            .message(scope.owner(), "hello", || {
                let mut args = Args::new();
                args.set("name", "baseline");
                args
            })
            .unwrap();
        return text().content(scope.bond(move || message.get().unwrap_or_default()));
    }
    #[cfg(feature = "use-resources")]
    {
        let resources = Resources::default();
        let value = resources
            .bind(
                scope.owner(),
                "sample",
                "v1",
                || 1_u32,
                |_| async { Ok::<_, String>("resource ready".to_owned()) },
            )
            .unwrap();
        scope.retain(resources).unwrap();
        return text().content(value.map(|state| match state {
            ResourceState::Ready { value, .. } => value.to_string(),
            _ => "pending".into(),
        }));
    }
    #[cfg(not(any(
        feature = "minimal",
        feature = "button",
        feature = "sapid-button",
        feature = "use-code",
        feature = "use-json",
        feature = "use-dates",
        feature = "use-files",
        feature = "use-chat",
        feature = "use-editor",
        feature = "use-charts",
        feature = "use-i18n",
        feature = "use-resources"
    )))]
    text().class("sample").content("hello")
}

#[wasm_bindgen]
pub fn mount() -> std::result::Result<(), JsValue> {
    #[cfg(feature = "base")]
    {
        let document = web_sys::window().unwrap().document().unwrap();
        let root = document.get_element_by_id("app").unwrap();
        #[cfg(feature = "use-css")]
        {
            let source = root
                .get_attribute("data-css")
                .unwrap_or_else(|| ".sample { opacity: 0.75; }".into());
            let sheet = glory::html::Sheet::parse(&source)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            let style = document.create_element("style")?;
            style.set_text_content(Some(
                &glory::html::sheet(&sheet, glory::render::style::Profile::HTML)
                    .map_err(|error| JsValue::from_str(&error.to_string()))?,
            ));
            document.document_element().unwrap().append_child(&style)?;
        }
        #[cfg(feature = "use-ssr")]
        root.set_attribute(
            "data-ssr",
            &glory::ssr::render(text().content("server output"))
                .map_err(|error| JsValue::from_str(&error.to_string()))?,
        )?;
        #[cfg(feature = "embed-stream-script")]
        root.set_attribute("data-stream-script", glory::web::STREAM_SCRIPT)?;
        #[cfg(feature = "use-stream")]
        let app = {
            let data = document.get_element_by_id("data").unwrap();
            let (app, resources) =
                glory::web::hydrate_resources_with(root, &data, Default::default(), |scope, _| {
                    ui(scope)
                })
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            app.app()
                .scope()
                .retain(resources)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
            app
        };
        #[cfg(all(feature = "use-hydrate", not(feature = "use-stream")))]
        let app = glory::web::hydrate_with(root, ui)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        #[cfg(not(any(feature = "use-hydrate", feature = "use-stream")))]
        let app = glory::web::launch_with(root, ui)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        APP.with(|slot| *slot.borrow_mut() = Some(app));
    }
    Ok(())
}

#[wasm_bindgen]
pub fn unmount() {
    #[cfg(feature = "base")]
    APP.with(|slot| drop(slot.borrow_mut().take()));
}
