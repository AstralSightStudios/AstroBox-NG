> **AstroBox vendored copy.** Linux builds of AstroBox run on this CEF
> (Chromium) runtime instead of WebKitGTK; other platforms keep wry. Taken from
> [SableClient/tauri-runtime-cef](https://github.com/SableClient/tauri-runtime-cef)
> at `6568170ae5fb27b38dae1e367f0bb3a09a040152` (examples, tests and scripts
> dropped). Local changes, marked `AstroBox:` in the source:
>
> - every `command_line_args` entry is appended as a switch; upstream appended
>   dash-less value-less entries as positional arguments, which silently dropped
>   switches such as its own `disable-gpu-process-crash-limit`;
> - the per-webview `user_agent` is applied through DevTools
>   (`Emulation.setUserAgentOverride`) instead of being ignored;
> - window transparency is ignored on Linux: winit's 32-bit ARGB visual makes
>   CEF's 24-bit child window creation fail with X11 `BadMatch`;
> - `winit` is pinned to `=0.31.0-beta.2` (beta.3 changed the drag-and-drop
>   event API).
>
> When updating, re-apply these and check the app-side setup in
> `src-tauri/modules/app/src/linux/cef.rs`.

# tauri-runtime-cef

A [CEF (Chromium Embedded Framework)](https://bitbucket.org/chromiumembedded/cef) runtime for [Tauri](https://tauri.app), maintained standalone so it builds against **published** tauri crates (`tauri-runtime`, `tauri-utils` from crates.io) — no tauri fork, no `[patch.crates-io]`.

Extracted from tauri's unreleased `feat/cef` branch (`tauri-apps/tauri@3b2823b91`, crate `crates/tauri-runtime-cef`) and ported to the published `tauri-runtime` trait surface. When upstream ships an official CEF runtime release, this repo retires.

## Usage

Published tauri has no `cef` feature, `tauri::Cef` alias, or `#[tauri::cef_entry_point]`; their equivalents live here:

```rust
type Cef = tauri_runtime_cef::CefRuntime<tauri::EventLoopMessage>;

fn main() {
    // Must run before anything else: CEF subprocesses re-exec this binary
    // and must register the same custom schemes as the browser process.
    tauri_runtime_cef::configure(tauri_runtime_cef::CefConfig {
        identifier: "com.example.app".into(),
        ..Default::default()
    });
    if std::env::args().any(|arg| arg.starts_with("--type=")) {
        tauri_runtime_cef::run_cef_helper_process();
        return;
    }

    tauri::Builder::<Cef>::default()
        // ...
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

Because published tauri only defaults its generic types (`AppHandle`, `WebviewWindow`, …) to wry, apps alias them once (`type AppHandle = tauri::AppHandle<Cef>;`) and build tauri with `default-features = false`.

## Additions

Capabilities this crate adds on top of the imported runtime:

- **Permission policy** (`set_permission_policy`): every Chromium permission request — media capture (`getUserMedia`/`getDisplayMedia`) and permission prompts (notifications, geolocation, clipboard, …) — is answered by a deny-by-default policy the app supplies. Highlights:
  - the policy sees runtime-neutral `PermissionKind`s and a `NormalizedOrigin` (scheme/host lowercased, default port resolved, opaque/`null`/host-less origins refused) instead of raw CEF bitmasks; CEF bits this crate does not know arrive as `PermissionKind::Unknown(bit)` so they can be denied rather than silently dropped;
  - **no policy set = nothing granted** (`DenyReason::NoPolicy`) — an app cannot forget its way into Chromium's own defaults;
  - the `PermissionResponder` completes the CEF callback **exactly once** on every path. It can `allow`, `deny`, `decide` per kind, or `defer(timeout)` for a native consent prompt — deferring keeps the CEF callback alive off the CEF thread. Dropping a responder, a prompt timing out (`DEFAULT_PROMPT_TIMEOUT`, 30s), or the webview closing all deny; nothing falls back to a grant;
  - per-kind verdicts collapse to an all-or-nothing grant because CEF requires it (`getUserMedia`'s granted mask must equal the requested one), so a partial verdict denies the request — the individual verdicts still reach the audit sink;
  - **audit sink** (`set_permission_audit`): one event per final decision, carrying the request id, webview label, origin, kinds, per-kind verdicts, and the deny reason — recorded after CEF is answered, so it reflects enforcement rather than intent.

  `NormalizedOrigin::is_app_local` is offered as a *transport* fact (custom schemes, localhost/loopback, `*.localhost`), not a trust verdict: an app that proxies remote content through a local origin — a gateway, a preview server — must distinguish those by webview label, since the origin cannot.

  The crate holds no policy of its own — it asks yours, before `tauri::Builder::run`:

  ```rust
  use tauri_runtime_cef::{DenyReason, PermissionKind, Verdict, DEFAULT_PROMPT_TIMEOUT};

  tauri_runtime_cef::set_permission_policy(|request, responder| {
      // Opaque, `null` and malformed origins do not normalize.
      let Some(origin) = request.origin.as_ref() else {
          return responder.deny(DenyReason::InvalidOrigin);
      };
      // Your own UI, on your own origin: grant what you declared, nothing else.
      if request.webview_label == "main" && origin.is_app_local() {
          let verdicts = request
              .kinds
              .iter()
              .map(|kind| match kind {
                  PermissionKind::Microphone | PermissionKind::Camera => Verdict::Allow,
                  _ => Verdict::Deny,
              })
              .collect();
          return responder.decide(verdicts);
      }
      // Embedded web content: ask the user, off the CEF thread. An unanswered
      // prompt, a dropped handle or a closing webview all deny on their own.
      let deferred = responder.defer(DEFAULT_PROMPT_TIMEOUT);
      show_your_native_consent_ui(request, deferred);
  });

  tauri_runtime_cef::set_permission_audit(|event| {
      eprintln!("permission {:?}: {:?}", event.granted, event.kinds);
  });
  ```

  The policy runs on a CEF thread and must not block: decide immediately, or `defer` and answer from your event loop.
- **Popup policy** (`set_popup_policy`): per-URL / per-webview-label `window.open` decisions.
- **Cache-lock fail-fast**: when another live instance of the app already holds the CEF cache's `SingletonLock`, runtime init returns an actionable error naming the holder pid and the fix (Chromium otherwise only surfaces the conflict later, as a renderer/GPU startup failure).
- **Exit codes**: `run_return` reports the code passed to exit requests.

## Port notes

How building against published tauri changes the mechanics, relative to the feat/cef branch this was extracted from:

- **Custom schemes** (`tauri://localhost`, `fetch("ipc://localhost/…")`, `asset://localhost`) are registered with Chromium as standard/secure/CORS/fetch-enabled schemes (`OnRegisterCustomSchemes`, list from `CefConfig::custom_schemes`), because published tauri emits the native scheme forms on Linux/macOS rather than feat/cef's `http(s)://<scheme>.localhost` mapping. Both forms are served.
- **Init data** (Chromium switches, cache path, identifier, deep-link schemes) comes from a process-global `CefConfig`; published `tauri-runtime` has no channel for runtime-specific init arguments.
- **`on_new_window` handlers act as a popup-deny signal** (unless a `set_popup_policy` hook decides otherwise): the published handler type wraps a wry platform webview handle that has no CEF equivalent, so the handler itself cannot be invoked — installing one denies popups, absence keeps CEF's native popup behavior.
- feat/cef trait surface that published tauri doesn't call yet (`go_back`/`go_forward` and friends) lives on as inherent methods on `CefWebviewDispatcher`; the address-changed and per-webview runtime-style channels are dormant until a tauri release ships them.

## Known ceilings

- Verified on Linux (X11). The macOS/Windows paths compile-ported blind — they need a real pass.
- macOS `.app` bundling needs the CEF framework + helper-app layout that feat/cef's tauri-cli produces; the published CLI doesn't do this. Bundle scripting lives with the consuming app for now.
- Deep-link relaunch URLs are dropped on Linux/Windows (published `tauri-runtime` has no `RunEvent::Opened` there).

## Native input test

Run `node scripts/check-input-focus.mjs` with Node 22+, Xvfb, xdotool, and cached
Rust dependencies. The test uses an isolated X11 display and a temporary profile.
Set `CEF_PATH` to the unpacked CEF distribution if its libraries are not in the
target's `debug` directory. `CARGO_TARGET_DIR` selects the build directory.

## License

Apache-2.0 OR MIT, same as upstream Tauri. Original code Copyright 2019-2024 Tauri Programme within The Commons Conservancy; see `LICENSE_APACHE-2.0`, `LICENSE_MIT`, and `LICENSE.spdx`.
