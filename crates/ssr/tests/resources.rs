#![cfg(feature = "resources")]

use glory_core::ResourceState;
use glory_resource::{Limits, Resources, Snapshot};
use glory_ui::{Widget, text};

fn resources(value: &str) -> Resources {
    let mut snapshot = Snapshot::default();
    snapshot
        .seed(
            "profile",
            "profile/1",
            &7,
            Ok::<_, &String>(&value.to_owned()),
        )
        .unwrap();
    Resources::restore(snapshot).unwrap()
}
fn ui(scope: &glory_ui::Scope, resources: &Resources) -> impl Widget + use<> {
    let value = resources
        .bind(
            scope.owner(),
            "profile",
            "profile/1",
            || 7,
            |_| async { Ok::<_, String>("network".to_owned()) },
        )
        .unwrap();
    text().content(value.map(|state| match state {
        ResourceState::Ready { value, .. } => value.to_string(),
        _ => "loading".into(),
    }))
}
#[test]
fn exports_before_app_cleanup_and_isolates_two_real_server_renders() {
    let a = resources("request A");
    let b = resources("request B");
    let page_a = glory_ssr::snapshot_resources_with(&a, |scope| ui(scope, &a)).unwrap();
    let page_b = glory_ssr::snapshot_resources_with(&b, |scope| ui(scope, &b)).unwrap();
    assert!(page_a.view.html.contains("request A"));
    assert!(!page_a.view.html.contains("request B"));
    assert!(page_b.view.html.contains("request B"));
    assert!(page_a.data.to_json().unwrap().contains("request A"));
    assert_eq!(page_a.data.len(), 1);
    assert!(a.is_empty() && b.is_empty()); // Real App drop already removed registrations.
    assert!(a.snapshot().unwrap().is_empty());
}
#[test]
fn script_data_preserves_json_without_an_html_raw_text_escape() {
    let value = "</ScRiPt><script>attack()</script><!--\u{2028}\u{2029}&\0末尾";
    let resources = resources(value);
    let page =
        glory_ssr::snapshot_resources_with(&resources, |scope| ui(scope, &resources)).unwrap();
    let script = glory_ssr::resource_script("data\"&", &page.data, Some("nonce\"<&")).unwrap();
    assert_eq!(script.matches("<script").count(), 1);
    assert_eq!(script.matches("</script>").count(), 1);
    assert!(script.contains("id=\"data&quot;&amp;\""));
    assert!(script.contains("nonce=\"nonce&quot;&lt;&amp;\""));
    let data = &script[script.find('>').unwrap() + 1..script.rfind("</script>").unwrap()];
    assert!(!data.contains('<'));
    assert!(!data.contains('\u{2028}'));
    assert!(!data.contains('&'));
    let snapshot = Snapshot::from_json(data, Limits::default()).unwrap();
    let restored = Resources::restore(snapshot).unwrap();
    let again =
        glory_ssr::snapshot_resources_with(&restored, |scope| ui(scope, &restored)).unwrap();
    assert_eq!(again.data.to_json().unwrap(), page.data.to_json().unwrap());
}
#[test]
fn escaped_html_payload_also_obeys_the_byte_limit() {
    let mut data = Snapshot::new(Limits {
        bytes: 200,
        records: 1,
    });
    data.seed("r", "v1", &0, Ok::<_, &String>(&"<".repeat(50)))
        .unwrap();
    assert!(matches!(
        glory_ssr::resource_script("data", &data, None),
        Err(glory_resource::Error::Limit("script data bytes"))
    ));
}
