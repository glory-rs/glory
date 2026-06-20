//! Integration coverage for `#[derive(glory::Store)]` field-level accessors.
//!
//! Exercises the generated `*Store` accessor and `*StoreExt::store()` entry
//! point against a real root [`Cage`], asserting that each field lens projects
//! the correct value, that writes through one field lens do not disturb the
//! others, and that nested stores compose.

use glory::reflow::{Cage, CageLens};

#[derive(Debug, glory::Store)]
struct Settings {
    volume: u32,
    name: String,
    muted: bool,
}

#[derive(Debug, glory::Store)]
struct App {
    settings: Settings,
    counter: i32,
}

#[test]
fn store_projects_each_field() {
    let cage = Cage::new(Settings {
        volume: 3,
        name: "main".to_owned(),
        muted: false,
    });
    let store = cage.store();

    assert_eq!(*store.volume().get_untracked(), 3);
    assert_eq!(&*store.name().get_untracked(), "main");
    assert!(!*store.muted().get_untracked());
}

#[test]
fn store_writes_are_field_independent() {
    let cage = Cage::new(Settings {
        volume: 3,
        name: "main".to_owned(),
        muted: false,
    });
    let store = cage.store();

    // Revise only `volume`.
    store.volume().revise(|mut v| *v = 11);
    assert_eq!(*store.volume().get_untracked(), 11);
    // Other fields untouched.
    assert_eq!(&*store.name().get_untracked(), "main");
    assert!(!*store.muted().get_untracked());

    // `set` returns the previous value and replaces in place.
    assert_eq!(store.name().set("alt".to_owned()), "main");
    assert_eq!(&*store.name().get_untracked(), "alt");

    store.muted().set(true);
    assert!(*store.muted().get_untracked());

    // Volume is still what we set earlier; cross-field writes stay isolated.
    assert_eq!(*store.volume().get_untracked(), 11);

    // The root cell reflects every projected write.
    let snapshot = cage.get_untracked();
    assert_eq!(snapshot.volume, 11);
    assert_eq!(snapshot.name, "alt");
    assert!(snapshot.muted);
}

#[test]
fn store_composes_nested_derive() {
    let cage = Cage::new(App {
        settings: Settings {
            volume: 1,
            name: "root".to_owned(),
            muted: true,
        },
        counter: 0,
    });
    let app = cage.store();

    // `settings()` yields a `CageLens<App, Settings>`; calling `.store()` on it
    // (via the per-type extension trait) produces a nested `SettingsStore<App>`.
    let settings: CageLens<App, Settings> = app.settings();
    let settings_store = settings.store();

    settings_store.volume().set(9);
    settings_store.name().set("nested".to_owned());
    app.counter().revise(|mut c| *c += 5);

    let snapshot = cage.get_untracked();
    assert_eq!(snapshot.settings.volume, 9);
    assert_eq!(snapshot.settings.name, "nested");
    assert!(snapshot.settings.muted);
    assert_eq!(snapshot.counter, 5);
}

#[test]
fn store_handle_clones_and_shares_root() {
    let cage = Cage::new(Settings {
        volume: 0,
        name: "x".to_owned(),
        muted: false,
    });
    let store = cage.store();
    let clone = store.clone();

    store.volume().set(42);
    // The clone shares the same root cell, so it observes the write.
    assert_eq!(*clone.volume().get_untracked(), 42);
    // `cage()` round-trips back to the originating root.
    assert_eq!(clone.cage().get_untracked().volume, 42);
}
