# Mobile Native Bridge Recipes

Date: 2026-06-12

Glory does not expose first-class camera, gallery, contacts, share-sheet, or
permission APIs yet. Keep those integrations in the generated mobile app until
real-device validation and user needs justify a stable wrapper.

## Recommended Shape

Use an app-owned bridge object and store its handle in `Truck`:

```rust
#[derive(Clone)]
pub struct MobileBridge {
    // App-defined request queue or platform callback handle.
}

impl MobileBridge {
    pub fn pick_photo(&self) {
        // Forward to Android/iOS host code.
    }
}
```

Widgets read the bridge from `Truck` and call it from event handlers. Native
callbacks should revise a `Cage` owned by the app or dispatch a typed
command-backend event with data in `EventData::extra`.

Keep platform handles out of reusable widgets. Pass plain app state, such as the
selected file URI or permission status, back into widgets through `Cage` values.

## Permissions

Android:

- declare required permissions in `android/app/src/main/AndroidManifest.xml`;
- request runtime permissions from the generated Activity or a small Kotlin
  helper;
- send the result back to Rust as an app-defined callback or webview custom
  event;
- store the result in a `Cage<PermissionState>`.

iOS:

- add the matching usage-description keys to the generated app plist;
- request access from Swift/Objective-C host code;
- send the result back to Rust using the same app bridge used for Android;
- keep denied/restricted states visible in app state so widgets can render a
  platform-neutral fallback.

## Camera Or Gallery Picker

Prefer returning a file URI, content URI, or copied app-cache path instead of raw
bytes. Large media payloads should stay outside `EventData`; pass a handle/path
through the bridge and let application code load it deliberately.

Android:

- use the Activity Result APIs from the generated Activity;
- request media permissions only when the selected Android API level requires
  them;
- copy content URIs to an app-owned cache file if the Rust side needs normal
  filesystem access.

iOS:

- use a PHPicker/UIImagePicker wrapper in the generated host;
- copy the selected asset to an app-cache URL before returning it to Rust when
  the Rust side needs file access.

## Share Sheet

Treat sharing as a command with no guaranteed result. Pass text, URL, or file
paths to host code; let Android/iOS present the native sheet and report only
coarse completion/cancel/error state back to the app.

## When To Promote To Glory API

Promote a recipe into a first-party helper only after:

- Android emulator/device smoke and iOS simulator/device smoke are both green;
- at least two examples need the same operation;
- the data shape can be platform-neutral without hiding important platform
  permission states.
