fn main() {
    let snapshot =
        glory::ssr::snapshot(glory::ui::text().class("sample").content("hello")).unwrap();
    let data =
        glory::ssr::resource_script("data", &glory::resources::Snapshot::default(), None).unwrap();
    println!(
        "{}",
        serde_json::json!({"html": snapshot.html, "data": data})
    );
}
