// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::sync::{Arc, mpsc::Sender};

use cef::*;
use tauri_runtime::{UserEvent, window::WindowId};
use winit::event_loop::EventLoopProxy as WinitEventLoopProxy;

use crate::runtime::{Message, RuntimeContext};

// There is some race condition on CEF that causes the app loading to fail
// when there is a network service crash:
// "[85296:47750637:0127/131203.017395:ERROR:content/browser/network_service_instance_impl.cc:610] Network service crashed or was terminated, restarting service."
// We check the app URL for a while until it actually loads the initial URL.
fn check_and_reload_if_blank(browser: cef::Browser, initial_url: String) {
  if initial_url == "about:blank" {
    return;
  }

  std::thread::spawn(move || {
    std::thread::sleep(std::time::Duration::from_secs(1));

    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);
    let check_interval = std::time::Duration::from_millis(100);

    while start_time.elapsed() < timeout {
      if let Some(frame) = browser.main_frame() {
        let url = frame.url();
        let current_url = cef::CefString::from(&url).to_string();
        if current_url.is_empty() || current_url == "about:blank" {
          frame.load_url(Some(&cef::CefString::from(initial_url.as_str())));
          // Continue checking in case it loads about:blank again.
        } else {
          return;
        }
      }
      std::thread::sleep(check_interval);
    }
  });
}

wrap_life_span_handler! {
  pub struct TauriCefChildLifeSpanHandler<T: UserEvent> {
    sender: Sender<Message<T>>,
    proxy: WinitEventLoopProxy,
    window_id: WindowId,
    webview_id: u32,
    webview_label: String,
    context: RuntimeContext<T>,
    new_window_handler: Option<Arc<crate::compat::NewWindowHandler>>,
    initial_url: Option<String>,
  }

  impl LifeSpanHandler {
    fn on_after_created(&self, browser: Option<&mut Browser>) {
      if let Some(browser) = browser
        && let Some(initial_url) = &self.initial_url
      {
        check_and_reload_if_blank(browser.clone(), initial_url.clone());
      }
    }

    fn on_before_popup(
      &self,
      _browser: Option<&mut Browser>,
      _frame: Option<&mut Frame>,
      _popup_id: std::os::raw::c_int,
      target_url: Option<&CefString>,
      _target_frame_name: Option<&CefString>,
      _target_disposition: WindowOpenDisposition,
      _user_gesture: std::os::raw::c_int,
      _popup_features: Option<&PopupFeatures>,
      _window_info: Option<&mut WindowInfo>,
      _client: Option<&mut Option<Client>>,
      _settings: Option<&mut BrowserSettings>,
      _extra_info: Option<&mut Option<DictionaryValue>>,
      _no_javascript_access: Option<&mut i32>,
    ) -> std::os::raw::c_int {
      // Return value: 0 = allow the popup, 1 = cancel it.
      // A crate-level popup policy (set_popup_policy) decides per URL/label
      // when installed.
      let url = target_url.map(|u| u.to_string()).unwrap_or_default();
      if let Some(allow) = crate::policy::popup_allowed(&crate::policy::PopupRequest {
        webview_label: &self.webview_label,
        url: &url,
      }) {
        return i32::from(!allow);
      }
      // ponytail: published tauri's new-window handler cannot be invoked from
      // CEF — its NewWindowFeatures wraps a wry platform webview handle
      // (webkit2gtk::WebView on Linux) that a CEF browser cannot construct.
      // An installed handler therefore degrades to a popup deny (the
      // verdict every current caller returns); no handler keeps CEF's native
      // popup behavior. Revisit when upstream releases feat/cef's
      // runtime-generic opener.
      i32::from(self.new_window_handler.is_some())
    }

    fn on_before_close(&self, browser: Option<&mut Browser>) {
      if browser.is_none() {
        return;
      }
      // Any permission prompt still open over this webview can no longer be
      // granted to — deny it rather than leave the callback (and the app's
      // consent UI) hanging over a dead browser.
      crate::policy::cancel_pending(&self.webview_label);
      let _ = self
        .sender
        .send(Message::BrowserClosed(self.window_id, self.webview_id));
      self.proxy.wake_up();
    }
  }
}
