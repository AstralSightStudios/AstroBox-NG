// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Process-global runtime configuration.
//!
//! Tauri's unreleased `feat/cef` branch threads CEF-specific init data
//! (command line switches, cache path, app identifier, custom schemes)
//! through a generic `RuntimeInitArgs`. Published tauri has no such channel,
//! so this crate takes the same data through a process-global set once by the
//! app before `tauri::Builder::run` — and, crucially, before
//! [`crate::run_cef_helper_process`], because CEF subprocesses re-exec the
//! app binary and must register the same custom schemes.
//!
//! ```no_run
//! tauri_runtime_cef::configure(tauri_runtime_cef::CefConfig {
//!   identifier: "com.example.app".into(),
//!   command_line_args: vec![("use-mock-keychain".into(), None)],
//!   ..Default::default()
//! });
//! ```

use std::path::PathBuf;
use std::sync::OnceLock;

/// Linux window system selected before CEF and the event loop start.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LinuxWindowing {
  #[default]
  X11,
  /// A CEF Views-owned top-level window using Ozone/Wayland.
  Wayland,
}

/// CEF runtime configuration, applied at `CefRuntime` creation (browser
/// process) and at custom-scheme registration (all processes).
#[derive(Debug, Clone)]
pub struct CefConfig {
  /// Application identifier; the CEF disk cache defaults to
  /// `{user cache dir}/{identifier}/cef`. Falls back to the executable name.
  pub identifier: String,
  /// Extra Chromium command line switches, `(name, optional value)`.
  pub command_line_args: Vec<(String, Option<String>)>,
  /// Overrides the CEF disk cache directory (`Settings::cache_path`).
  pub cache_path: Option<PathBuf>,
  /// Deep link schemes to register as protocol handlers.
  pub deep_link_schemes: Vec<String>,
  /// Tauri custom protocol schemes to register with Chromium as standard,
  /// secure, CORS- and fetch-enabled schemes so published tauri's native URL
  /// forms (`tauri://localhost`, `fetch("ipc://localhost/…")`) work under
  /// CEF. Must cover every scheme the app or its plugins register.
  pub custom_schemes: Vec<String>,
  /// Custom schemes whose origins get a cookie jar
  /// (`cef::Settings::cookieable_schemes_list`). CEF cookies only http/https
  /// by default, so `document.cookie` and `Set-Cookie` are inert on custom
  /// schemes not listed here. The http/https defaults are preserved.
  pub cookieable_schemes: Vec<String>,
  /// Linux window system. Ignored on other platforms.
  pub linux_windowing: LinuxWindowing,
}

impl Default for CefConfig {
  fn default() -> Self {
    Self {
      identifier: std::env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "tauri-app".into()),
      command_line_args: Vec::new(),
      cache_path: None,
      deep_link_schemes: Vec::new(),
      custom_schemes: vec!["tauri".into(), "ipc".into(), "asset".into()],
      cookieable_schemes: Vec::new(),
      linux_windowing: LinuxWindowing::X11,
    }
  }
}

static CONFIG: OnceLock<CefConfig> = OnceLock::new();

/// Sets the process-global CEF configuration. Call at the very top of
/// `main`, before [`crate::run_cef_helper_process`] and before building the
/// tauri app. Later calls are ignored.
pub fn configure(config: CefConfig) {
  let _ = CONFIG.set(config);
}

pub(crate) fn config() -> &'static CefConfig {
  CONFIG.get_or_init(CefConfig::default)
}

#[cfg(target_os = "linux")]
pub(crate) fn native_wayland() -> bool {
  config().linux_windowing == LinuxWindowing::Wayland
}
