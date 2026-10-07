//! Creates and configures the cef webview, which renders the content
//! Implements tauri operations on the cef webview

use std::{
  collections::HashMap,
  sync::{Arc, Mutex, atomic::Ordering, mpsc},
};

use cef::*;
use tauri_runtime::{
  Error, UserEvent,
  dpi::{PhysicalPosition, Rect},
  webview::{PendingWebview, WebviewAttributes},
  window::WindowId,
};
use tauri_utils::Theme;

use crate::{
  cef_impl::{client as browser_client, request_context, request_handler},
  compat::UriSchemeProtocolHandler,
  runtime::{CefRuntime, RuntimeContext},
  webview::{
    AppWebview, INITIAL_LOAD_URL, PendingInitialLoads, Webview, WebviewMessage,
    add_dev_tools_observer, initialization_scripts,
    load_initial_url_after_registering_initialization_scripts,
  },
};

use super::window::{NativeWindow, WaylandWindow, WindowConfig};

fn browser_settings(attrs: &WebviewAttributes) -> BrowserSettings {
  BrowserSettings {
    javascript: State::from(if attrs.javascript_disabled {
      sys::cef_state_t::STATE_DISABLED
    } else {
      sys::cef_state_t::STATE_ENABLED
    }),
    javascript_access_clipboard: State::from(if attrs.clipboard {
      sys::cef_state_t::STATE_ENABLED
    } else {
      sys::cef_state_t::STATE_DISABLED
    }),
    chrome_status_bubble: State::from(sys::cef_state_t::STATE_DISABLED),
    ..Default::default()
  }
}

pub(super) fn build<T: UserEvent>(
  context: &RuntimeContext<T>,
  scheme_registry: &request_handler::SchemeRegistry,
  window_id: WindowId,
  webview_id: u32,
  config: WindowConfig,
  theme: Option<Theme>,
  mut pending: PendingWebview<T, CefRuntime<T>>,
) -> Option<(AppWebview, NativeWindow)> {
  pending.webview_attributes.drag_drop_handler_enabled = false;
  let scripts = initialization_scripts(&mut pending.webview_attributes);
  let uri_scheme_protocols: Arc<HashMap<String, Arc<Box<UriSchemeProtocolHandler>>>> = Arc::new(
    pending
      .uri_scheme_protocols
      .into_iter()
      .map(|(scheme, handler)| (scheme, Arc::new(handler)))
      .collect(),
  );
  let handlers = browser_client::TauriCefBrowserClientHandlers {
    ipc_handler: pending.ipc_handler.map(Arc::from),
    on_page_load_handler: pending.on_page_load_handler.take().map(Arc::from),
    document_title_changed_handler: pending.document_title_changed_handler.take().map(Arc::from),
    navigation_handler: pending.navigation_handler.map(Arc::from),
    address_changed_handler: None,
    new_window_handler: Some(Arc::new(|_, _| {
      tauri_runtime::webview::NewWindowResponse::Deny
    })),
    download_handler: pending.download_handler.take(),
    web_content_process_terminate_handler: None,
  };
  let mut client = browser_client::TauriCefBrowserClient::new(
    context.clone(),
    window_id,
    webview_id,
    pending.label.clone(),
    Some(pending.url.as_str().to_string()),
    (cfg!(debug_assertions) || cfg!(feature = "devtools"))
      && pending.webview_attributes.devtools.unwrap_or(true),
    browser_client::DragDropEventTarget::Window,
    false,
    Arc::new(Mutex::new(browser_client::DragDropState::default())),
    handlers,
    context.proxy.clone(),
    context.sender.clone(),
  );
  let settings = browser_settings(&pending.webview_attributes);
  let custom_protocol_scheme = if pending.webview_attributes.use_https_scheme {
    "https"
  } else {
    "http"
  }
  .to_string();
  let custom_scheme_domains = uri_scheme_protocols
    .keys()
    .map(|scheme| format!("{scheme}.localhost"))
    .collect::<Vec<_>>();
  let real_initial_url = pending.url.as_str().to_string();
  let user_agent = pending.webview_attributes.user_agent.clone();
  let label = pending.label.clone();
  let (browser_tx, browser_rx) = mpsc::channel();
  let (init_done, on_initialized) = request_context::deferred_init_continuation({
    let scheme_registry = scheme_registry.clone();
    let uri_scheme_protocols = uri_scheme_protocols.clone();
    let scripts = scripts.clone();
    let custom_protocol_scheme = custom_protocol_scheme.clone();
    let custom_scheme_domains = custom_scheme_domains.clone();
    move |mut request_context| {
      request_context::apply_theme_scheme(request_context.as_ref(), theme);
      let callback_label = label.clone();
      let callback_protocols = uri_scheme_protocols.clone();
      let callback_scripts = scripts.clone();
      let callback_registry = scheme_registry.clone();
      let callback_scheme = custom_protocol_scheme.clone();
      let callback_domains = custom_scheme_domains.clone();
      let callback_url = real_initial_url.clone();
      let callback_user_agent = user_agent.clone();
      if super::window::create(
        &mut client,
        &CefString::from(INITIAL_LOAD_URL),
        &settings,
        request_context.as_mut(),
        config,
        Box::new(move |browser, native| {
          let Some(host) = browser.host() else {
            log::error!("CEF browser for webview {callback_label:?} has no host");
            return;
          };
          let browser_id = browser.identifier();
          {
            let mut registry = callback_registry.lock().unwrap();
            for (scheme, handler) in callback_protocols.iter() {
              registry.insert(
                (browser_id, scheme.clone()),
                (
                  callback_label.clone(),
                  handler.clone(),
                  callback_scripts.clone(),
                ),
              );
            }
          }
          let protocol_handlers = Arc::new(Mutex::new(Vec::new()));
          let pending_loads: PendingInitialLoads = Arc::new(Mutex::new(HashMap::new()));
          let registration = Arc::new(Mutex::new(add_dev_tools_observer(
            &browser,
            protocol_handlers.clone(),
            pending_loads.clone(),
          )));
          load_initial_url_after_registering_initialization_scripts(
            &browser,
            &callback_scripts,
            &callback_scheme,
            &callback_domains,
            &callback_url,
            callback_user_agent.as_deref(),
            &pending_loads,
          );
          let _ = browser_tx.send((
            AppWebview {
              webview_id,
              label: callback_label,
              browser,
              browser_id,
              host,
              uri_scheme_protocols: callback_protocols,
              devtools_protocol_handlers: protocol_handlers,
              devtools_observer_registration: registration,
              listeners: Default::default(),
              bounds_rate: None,
            },
            native,
          ));
        }),
      )
      .is_none()
      {
        log::error!("failed to create CEF Views window for webview {label:?}");
      }
    }
  });
  let request_context = request_context::request_context_from_webview_attributes(
    &context.cache_path,
    &pending.webview_attributes,
    uri_scheme_protocols.keys(),
    &custom_protocol_scheme,
    scheme_registry.clone(),
    on_initialized,
  );
  if request_context.is_none() {
    init_done.store(true, Ordering::SeqCst);
  }
  request_context::wait_for_deferred_init(&init_done);
  wait_for_result(&browser_rx)
}

fn wait_for_result<T>(receiver: &mpsc::Receiver<T>) -> Option<T> {
  if cef::currently_on(sys::cef_thread_id_t::TID_UI.into()) == 0 {
    return receiver.recv().ok();
  }
  let _allow = request_context::AllowNestableTasks::enter();
  loop {
    match receiver.try_recv() {
      Ok(value) => return Some(value),
      Err(mpsc::TryRecvError::Disconnected) => return None,
      Err(mpsc::TryRecvError::Empty) => cef::do_message_loop_work(),
    }
  }
}

fn browser_view(child: &AppWebview) -> Option<BrowserView> {
  browser_view_get_for_browser(Some(&mut child.browser.clone()))
}

pub(super) fn take_focus(child: &AppWebview) {
  if let Some(view) = browser_view(child) {
    view.request_focus();
  }
}

pub(super) fn handle_message(
  window_id: WindowId,
  window: &mut WaylandWindow,
  message: WebviewMessage,
) {
  let size = window.initial_surface_size;
  let Some(child) = window.child.as_mut() else {
    return;
  };
  match message {
    WebviewMessage::AddEventListener(..) => {}
    WebviewMessage::EvaluateScript(script) => {
      if let Some(frame) = child.browser.main_frame() {
        frame.execute_java_script(Some(&script.as_str().into()), Some(&"".into()), 0);
      }
    }
    WebviewMessage::EvaluateScriptWithCallback(..) => {
      log::error!("eval_with_callback is unimplemented for native Wayland; dropping callback");
    }
    WebviewMessage::Navigate(url) => {
      if let Some(frame) = child.browser.main_frame() {
        frame.load_url(Some(&url.as_str().into()));
      }
    }
    WebviewMessage::Reload => child.browser.reload(),
    WebviewMessage::GoBack => child.browser.go_back(),
    WebviewMessage::CanGoBack(tx) => _ = tx.send(Ok(child.browser.can_go_back() == 1)),
    WebviewMessage::GoForward => child.browser.go_forward(),
    WebviewMessage::CanGoForward(tx) => _ = tx.send(Ok(child.browser.can_go_forward() == 1)),
    WebviewMessage::Print => child.host.print(),
    WebviewMessage::Close => child.host.close_browser(1),
    WebviewMessage::Show => {
      if let Some(view) = browser_view(child) {
        view.set_visible(1);
      }
    }
    WebviewMessage::Hide => {
      if let Some(view) = browser_view(child) {
        view.set_visible(0);
      }
    }
    WebviewMessage::SetPosition(_) | WebviewMessage::SetSize(_) | WebviewMessage::SetBounds(_) => {}
    WebviewMessage::SetFocus => {
      child.host.set_focus(1);
      take_focus(child);
    }
    WebviewMessage::Reparent(target, tx) => {
      _ = tx.send(if target == window_id {
        Ok(())
      } else {
        Err(Error::WindowNotFound)
      });
    }
    WebviewMessage::SetAutoResize(_) | WebviewMessage::ClearAllBrowsingData => {}
    WebviewMessage::SetZoom(factor) => {
      child.host.set_zoom_level(if factor > 0.0 {
        factor.ln() / 1.2_f64.ln()
      } else {
        0.0
      });
    }
    WebviewMessage::SetBackgroundColor(_) => {}
    WebviewMessage::Url(tx) => _ = tx.send(Ok(child.url().unwrap_or_default())),
    WebviewMessage::Bounds(tx) => {
      _ = tx.send(Ok(Rect {
        position: PhysicalPosition::new(0, 0).into(),
        size: size.into(),
      }))
    }
    WebviewMessage::Position(tx) => _ = tx.send(Ok(PhysicalPosition::new(0, 0))),
    WebviewMessage::Size(tx) => _ = tx.send(Ok(size)),
    WebviewMessage::WithWebview(callback) => callback(Webview::new(child.browser.clone())),
    WebviewMessage::CookiesForUrl(_, tx) | WebviewMessage::Cookies(tx) => {
      _ = tx.send(Ok(Vec::new()))
    }
    WebviewMessage::SetCookie(_) | WebviewMessage::DeleteCookie(_) => {}
    #[cfg(any(debug_assertions, feature = "devtools"))]
    WebviewMessage::OpenDevTools => child.host.show_dev_tools(None, None, None, None),
    #[cfg(any(debug_assertions, feature = "devtools"))]
    WebviewMessage::CloseDevTools => child.host.close_dev_tools(),
    #[cfg(any(debug_assertions, feature = "devtools"))]
    WebviewMessage::IsDevToolsOpen(tx) => _ = tx.send(child.host.has_dev_tools() == 1),
    WebviewMessage::SendDevToolsMessage(_, tx) | WebviewMessage::OnDevToolsProtocol(_, tx) => {
      _ = tx.send(Err(Error::FailedToSendMessage))
    }
  }
}
