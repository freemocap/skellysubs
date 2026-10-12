# Tao mobile startup backports

This is the crates.io `tao` 0.35.3 source, licensed under Apache-2.0, with targeted
Android and iOS startup corrections. Original LICENSE, LICENSE.spdx and source headers
are retained. Cargo cache metadata and the dependency's own lockfile are omitted.

The source archive checksum in the original application lockfile is
`d1c93047acf68669466a34690ac58cca7010bd1b201e1ec86f1fd0a75d3dd4a9`.

`src/platform_impl/android/mod.rs` backports upstream
[beb88249 / PR 1304](https://github.com/tauri-apps/tao/commit/beb88249):
drain the EventLoopProxy user-event queue after every poll result, not only an
explicit wake result. Android may report another ready event while consuming the
wake signal, leaving IPC responses queued indefinitely. The backport retains
0.35.3's event-handler macro and APIs.

Tauri runtime 2.11.4 requires Tao 0.35, while this upstream fix shipped in Tao
0.37.0. The application uses a Cargo path patch so ordinary local and CI/release
builds all include the correction without editing a developer's Cargo cache.
Remove this copy and the Cargo patch when the supported Tauri runtime accepts a
Tao release containing these fixes, and rerun both mobile startup suites.

Device reproduction and verification are recorded in
[the investigation](../../../../docs/notes/android-startup-white-screen-2026-10-12.md).
Use `npm run e2e:android:startup` against an installed release-equivalent APK;
debug WebView reload tests cannot prove cold-start correctness.

## iOS scene lifecycle

`src/platform_impl/ios/{scene,app_state,view}.rs` adopts the manifest-presence
policy from [a3ff3f03](https://github.com/tauri-apps/tao/commit/a3ff3f03), so a
single-window app declaring scenes gets scene setup and window attachment.
This is a scoped adaptation to 0.35.3, not the entire upstream change: the app
always supplies an explicit scene manifest, and no support for implicitly
connected scenes without a manifest is added. Multiple-window permission is
still read separately from UIKit's `supportsMultipleScenes`.

The configuration callback returns `Retained::autorelease_ptr(config)`, matching
[PR 1245](https://github.com/tauri-apps/tao/pull/1245), instead of returning a pointer
to an object released at the end of the callback. The delegate class is registered
before UIKit resolves the static manifest name. `native/Info.ios.plist` supplies
the matching `TaoScene` / `TaoSceneDelegate` declaration with multiple scenes
explicitly disabled. Desktop platform code is unchanged.
