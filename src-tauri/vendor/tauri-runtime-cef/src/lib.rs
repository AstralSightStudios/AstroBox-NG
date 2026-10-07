// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

#![allow(clippy::arc_with_non_send_sync)]
#![allow(clippy::too_many_arguments)]

mod cef_impl;
mod compat;
mod config;
mod external_message_pump;
mod platform;
mod policy;
mod runtime;
mod streaming;
#[cfg(target_os = "linux")]
mod wayland;
mod webview;
mod window;
mod window_builder;
mod window_handle;

pub use config::{CefConfig, LinuxWindowing, configure};
#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
pub use platform::linux::install_x_error_handlers;
pub use policy::{
  DEFAULT_PROMPT_TIMEOUT, DeferredResponder, DenyReason, NormalizedOrigin, PermissionAudit,
  PermissionKind, PermissionRequest, PermissionResponder, PopupRequest, RequestSource, Verdict,
  set_permission_audit, set_permission_policy, set_popup_policy,
};
pub use runtime::*;
pub use streaming::{
  InitiatorOrigin, StreamClosed, StreamResponder, StreamWriter, register_streaming_scheme_handler,
};
pub use webview::*;
pub use window::CefWindowDispatcher;
pub use window_builder::WindowBuilderWrapper;
