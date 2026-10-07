// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Local stand-ins for items that exist on tauri's unreleased `feat/cef`
//! branch but not in the published `tauri-runtime`/`tauri-utils` crates this
//! standalone crate builds against. Each item mirrors the upstream shape so
//! the rest of the crate reads identically to its feat/cef ancestor; delete
//! entries as upstream releases catch up.

use std::borrow::Cow;

use tauri_runtime::Icon;
#[cfg(target_os = "macos")]
use tauri_runtime::dpi::LogicalRect;
#[cfg(not(target_os = "macos"))]
use tauri_runtime::dpi::PhysicalRect;
use tauri_runtime::dpi::{Pixel, Rect};
use tauri_runtime::webview::{DownloadEvent, PageLoadEvent};
use url::Url;

// Published tauri-runtime keeps these handler aliases private; the types
// themselves (boxed fields on PendingWebview) are identical.
pub(crate) type UriSchemeProtocolHandler = dyn Fn(&str, http::Request<Vec<u8>>, Box<dyn FnOnce(http::Response<Cow<'static, [u8]>>) + Send>)
  + Send
  + Sync
  + 'static;

pub(crate) type NavigationHandler = dyn Fn(&Url) -> bool + Send;

pub(crate) type OnPageLoadHandler = dyn Fn(Url, PageLoadEvent) + Send;

pub(crate) type DocumentTitleChangedHandler = dyn Fn(String) + Send + 'static;

pub(crate) type DownloadHandler = dyn Fn(DownloadEvent) -> bool + Send + Sync;

// Mirrors the published (wry-shaped) alias. The handler can be stored but
// never invoked from CEF: constructing the published `NewWindowFeatures`
// requires a platform webview handle (`webkit2gtk::WebView` on Linux) that a
// CEF browser cannot provide. See `life_span.rs` for how presence is used.
pub(crate) type NewWindowHandler = dyn Fn(Url, tauri_runtime::webview::NewWindowFeatures) -> tauri_runtime::webview::NewWindowResponse
  + Send;

// feat/cef-only: published PendingWebview has no address-changed channel, so
// this stays crate-internal (currently never populated) until upstream ships.
pub(crate) type AddressChangedHandler = dyn Fn(&Url) + Send + Sync + 'static;

// feat/cef adds these conversions on tauri_runtime::dpi::Rect directly.
#[cfg(not(target_os = "macos"))]
pub(crate) fn rect_to_physical<P: Pixel, S: Pixel>(rect: Rect, scale: f64) -> PhysicalRect<P, S> {
  PhysicalRect {
    position: rect.position.to_physical(scale),
    size: rect.size.to_physical(scale),
  }
}

#[cfg(target_os = "macos")]
pub(crate) fn rect_to_logical<P: Pixel, S: Pixel>(rect: Rect, scale: f64) -> LogicalRect<P, S> {
  LogicalRect {
    position: rect.position.to_logical(scale),
    size: rect.size.to_logical(scale),
  }
}

// feat/cef adds Icon::into_owned.
pub(crate) fn icon_into_owned(icon: Icon<'_>) -> Icon<'static> {
  Icon {
    rgba: Cow::Owned(icon.rgba.into_owned()),
    width: icon.width,
    height: icon.height,
  }
}
