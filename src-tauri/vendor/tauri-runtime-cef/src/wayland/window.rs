//! This wires up the window created by cef to tauri
//! For context: In the X11 linux version, winit's top level window is wired to tauri

use std::{
  collections::HashMap,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  },
};

use cef::{rc::Rc, *};
use tauri_runtime::{
  Error, WindowEventId,
  dpi::{PhysicalPosition, PhysicalSize, Size as TauriSize},
  window::{WindowEvent, WindowId},
};
use winit::event_loop::ActiveEventLoop;

use crate::{
  webview::AppWebview,
  window::{AppWindowAttrs, WindowMessage, winit_theme_to_tauri_theme},
};

type BrowserCreated = Box<dyn FnOnce(Browser, NativeWindow)>;
type Emit = Arc<dyn Fn(Event) + Send + Sync>;

#[derive(Debug)]
pub(super) enum Event {
  CloseRequested,
  Destroyed,
  Focused(bool),
}

#[derive(Clone)]
pub(super) struct WindowConfig {
  title: String,
  bounds: Rect,
  show_state: ShowState,
  visible: bool,
  frameless: bool,
  initial_scale_factor: f64,
  app_id: String,
  emit: Emit,
  resizable: bool,
  maximizable: bool,
  minimizable: bool,
  closable: bool,
  min_size: Option<TauriSize>,
}

impl WindowConfig {
  pub(super) fn new(
    attrs: &AppWindowAttrs,
    size: PhysicalSize<u32>,
    scale_factor: f64,
    emit: impl Fn(Event) + Send + Sync + 'static,
  ) -> Self {
    let buttons = attrs.inner.enabled_buttons;
    let show_state = if attrs.inner.fullscreen.is_some() {
      ShowState::FULLSCREEN
    } else if attrs.inner.maximized {
      ShowState::MAXIMIZED
    } else {
      ShowState::NORMAL
    };

    Self {
      title: attrs.inner.title.clone(),
      bounds: Rect {
        x: 0,
        y: 0,
        width: cef_dimension(size.width, scale_factor),
        height: cef_dimension(size.height, scale_factor),
      },
      show_state,
      visible: attrs.inner.visible,
      frameless: !attrs.inner.decorations,
      initial_scale_factor: scale_factor,
      app_id: crate::config::config().identifier.clone(),
      emit: Arc::new(emit),
      resizable: attrs.inner.resizable,
      maximizable: buttons.contains(winit::window::WindowButtons::MAXIMIZE),
      minimizable: buttons.contains(winit::window::WindowButtons::MINIMIZE),
      closable: buttons.contains(winit::window::WindowButtons::CLOSE),
      min_size: attrs.inner.min_surface_size,
    }
  }
}

pub(crate) struct NativeWindow {
  window: Window,
  browser_view: BrowserView,
  allow_close: Arc<AtomicBool>,
}

impl NativeWindow {
  pub(super) fn force_close(&self) {
    self.allow_close.store(true, Ordering::Release);
    self.window.close();
  }
}

type WindowEventListener = Box<dyn Fn(&WindowEvent) + Send>;

pub(super) struct WaylandWindow {
  pub(super) id: WindowId,
  pub(super) label: String,
  pub(super) attrs: AppWindowAttrs,
  pub(super) child: Option<AppWebview>,
  pub(super) listeners: Arc<Mutex<HashMap<WindowEventId, WindowEventListener>>>,
  pub(super) native: Option<NativeWindow>,
  pub(super) initial_surface_size: PhysicalSize<u32>,
  pub(super) initial_scale_factor: f64,
  pub(super) reported_focus: bool,
}

impl WaylandWindow {
  pub(super) fn close(&self) {
    if let Some(native) = &self.native {
      native.force_close();
    }
  }
}

pub(crate) fn handle_window_message(
  appwindow: &mut WaylandWindow,
  event_loop: &dyn ActiveEventLoop,
  message: WindowMessage,
) {
  let native = appwindow.native.as_ref();
  let buttons = appwindow.attrs.inner.enabled_buttons;

  match message {
    WindowMessage::AddEventListener(id, listener) => {
      appwindow.listeners.lock().unwrap().insert(id, listener);
    }
    WindowMessage::Close | WindowMessage::Destroy => {
      unreachable!("handled before borrowing")
    }
    WindowMessage::ScaleFactor(tx) => _ = tx.send(Ok(appwindow.initial_scale_factor)),
    WindowMessage::InnerPosition(tx) | WindowMessage::OuterPosition(tx) => {
      _ = tx.send(Ok(PhysicalPosition::new(0, 0)))
    }
    WindowMessage::InnerSize(tx) | WindowMessage::OuterSize(tx) => {
      _ = tx.send(Ok(appwindow.initial_surface_size))
    }
    WindowMessage::IsFullscreen(tx) => {
      _ = tx.send(Ok(
        native
          .map(|native| native.window.is_fullscreen() != 0)
          .unwrap_or(appwindow.attrs.inner.fullscreen.is_some()),
      ))
    }
    WindowMessage::IsMinimized(tx) => {
      _ = tx.send(Ok(
        native.is_some_and(|native| native.window.is_minimized() != 0),
      ))
    }
    WindowMessage::IsMaximized(tx) => {
      _ = tx.send(Ok(
        native
          .map(|native| native.window.is_maximized() != 0)
          .unwrap_or(appwindow.attrs.inner.maximized),
      ))
    }
    WindowMessage::IsFocused(tx) => {
      _ = tx.send(Ok(
        native.is_some_and(|native| native.window.is_active() != 0),
      ))
    }
    WindowMessage::IsDecorated(tx) => _ = tx.send(Ok(appwindow.attrs.inner.decorations)),
    WindowMessage::IsResizable(tx) => _ = tx.send(Ok(appwindow.attrs.inner.resizable)),
    WindowMessage::IsMaximizable(tx) => {
      _ = tx.send(Ok(buttons.contains(winit::window::WindowButtons::MAXIMIZE)))
    }
    WindowMessage::IsMinimizable(tx) => {
      _ = tx.send(Ok(buttons.contains(winit::window::WindowButtons::MINIMIZE)))
    }
    WindowMessage::IsClosable(tx) => {
      _ = tx.send(Ok(buttons.contains(winit::window::WindowButtons::CLOSE)))
    }
    WindowMessage::IsVisible(tx) => {
      _ = tx.send(Ok(
        native
          .map(|native| native.window.is_visible() != 0)
          .unwrap_or(appwindow.attrs.inner.visible),
      ))
    }
    WindowMessage::IsEnabled(tx) => _ = tx.send(Ok(true)),
    WindowMessage::IsAlwaysOnTop(tx) => _ = tx.send(Ok(false)),
    WindowMessage::Title(tx) => _ = tx.send(Ok(appwindow.attrs.inner.title.clone())),
    // Monitor queries are unimplemented for native Wayland: no consumer calls
    // them, and there's no Wayland protocol for the work_area they'd need.
    WindowMessage::CurrentMonitor(tx)
    | WindowMessage::PrimaryMonitor(tx)
    | WindowMessage::MonitorFromPoint(tx, ..) => _ = tx.send(Ok(None)),
    WindowMessage::AvailableMonitors(tx) => _ = tx.send(Ok(Vec::new())),
    WindowMessage::RawWindowHandle(tx) => _ = tx.send(Err(Error::FailedToSendMessage)),
    WindowMessage::Theme(tx) => {
      let theme = appwindow
        .attrs
        .inner
        .preferred_theme
        .or_else(|| event_loop.system_theme())
        .map(winit_theme_to_tauri_theme)
        .unwrap_or(tauri_utils::Theme::Light);
      _ = tx.send(Ok(theme));
    }
    WindowMessage::SetTitle(title) => {
      appwindow.attrs.inner.title = title.clone();
      if let Some(native) = native {
        native
          .window
          .set_title(Some(&CefString::from(title.as_str())));
      }
    }
    WindowMessage::Maximize => {
      appwindow.attrs.inner.maximized = true;
      if let Some(native) = native {
        native.window.maximize();
      }
    }
    WindowMessage::Unmaximize => {
      appwindow.attrs.inner.maximized = false;
      if let Some(native) = native {
        native.window.restore();
      }
    }
    WindowMessage::Minimize => {
      if let Some(native) = native {
        native.window.minimize();
      }
    }
    WindowMessage::Unminimize => {
      if let Some(native) = native {
        native.window.restore();
      }
    }
    WindowMessage::Show => {
      appwindow.attrs.inner.visible = true;
      if let Some(native) = native {
        native.window.show();
      }
    }
    WindowMessage::Hide => {
      appwindow.attrs.inner.visible = false;
      if let Some(native) = native {
        native.window.hide();
      }
    }
    WindowMessage::SetFullscreen(fullscreen) => {
      appwindow.attrs.inner.fullscreen =
        fullscreen.then_some(winit::monitor::Fullscreen::Borderless(None));
      if let Some(native) = native {
        native.window.set_fullscreen(i32::from(fullscreen));
      }
    }
    WindowMessage::SetFocus => {
      if let Some(native) = native {
        native.window.activate();
        native.browser_view.request_focus();
      }
    }
    WindowMessage::Center
    | WindowMessage::RequestUserAttention(_)
    | WindowMessage::SetEnabled(_)
    | WindowMessage::SetResizable(_)
    | WindowMessage::SetMaximizable(_)
    | WindowMessage::SetMinimizable(_)
    | WindowMessage::SetClosable(_)
    | WindowMessage::SetDecorations(_)
    | WindowMessage::SetAlwaysOnBottom(_)
    | WindowMessage::SetAlwaysOnTop(_)
    | WindowMessage::SetVisibleOnAllWorkspaces(_)
    | WindowMessage::SetContentProtected(_)
    | WindowMessage::SetMinSize(_)
    | WindowMessage::SetMaxSize(_)
    | WindowMessage::SetSizeConstraints(_)
    | WindowMessage::SetFocusable(_)
    | WindowMessage::SetIcon(_)
    | WindowMessage::SetSkipTaskbar(_)
    | WindowMessage::SetShadow(_)
    | WindowMessage::SetCursorGrab(_)
    | WindowMessage::SetCursorVisible(_)
    | WindowMessage::SetCursorIcon(_)
    | WindowMessage::SetCursorPosition(_)
    | WindowMessage::SetIgnoreCursorEvents(_)
    | WindowMessage::SetBadgeCount(..)
    | WindowMessage::SetBadgeLabel(_)
    | WindowMessage::SetOverlayIcon(_)
    | WindowMessage::SetTitleBarStyle(_)
    | WindowMessage::SetTrafficLightPosition(_)
    | WindowMessage::SetProgressBar(_)
    | WindowMessage::SetTheme(_) => {}
    // ponytail: no draggable-region support on the CEF Views backend, so a frameless
    // window cannot be moved or edge-resized from the page. Only reachable when
    // `decorations(false)` is set at build time. Upgrade path: implement
    // `DragHandler::on_draggable_regions_changed` -> `Window::set_draggable_regions`
    // and inject the `-webkit-app-region` stylesheet (see `src/native_wayland/drag.rs`
    // in commit ddd270e).
    // ponytail: geometry is frozen at creation. Getters answer from
    // `initial_surface_size` / `PhysicalPosition(0, 0)` and the setters do nothing, so
    // `tauri-plugin-window-state` will persist the creation size at 0,0. Upgrade path
    // for size: `native.window.size()` / `set_bounds()` (already in scope, see
    // `IsFullscreen` above). Position is a genuine Wayland protocol limit -- there are
    // no global window coordinates -- so it stays stubbed.
    WindowMessage::SetSize(_) | WindowMessage::SetPosition(_) => {
      log::warn!(
        "set_size/set_position are unsupported on native Wayland; window geometry is \
         fixed at the creation size and any persisted window state will be inaccurate"
      );
    }
    WindowMessage::StartDragging | WindowMessage::StartResizeDragging(_) => {
      log::warn!(
        "window drag is unsupported on native Wayland (decorations={}); \
         the CEF Views backend does not implement draggable regions",
        appwindow.attrs.inner.decorations
      );
    }
    WindowMessage::SetBackgroundColor(_) => {}
  }
}

fn cef_dimension(value: u32, scale: f64) -> i32 {
  (value as f64 / scale).round().max(1.0) as i32
}

fn cef_size_from_tauri(size: TauriSize, scale: f64) -> Size {
  let size = size.to_physical::<u32>(scale);
  Size {
    width: cef_dimension(size.width, scale),
    height: cef_dimension(size.height, scale),
  }
}

wrap_browser_view_delegate! {
  struct NativeBrowserViewDelegate {
    on_created: Arc<Mutex<Option<BrowserCreated>>>,
    allow_close: Arc<AtomicBool>,
  }

  impl ViewDelegate {}

  impl BrowserViewDelegate {
    fn browser_runtime_style(&self) -> RuntimeStyle {
      RuntimeStyle::CHROME
    }

    fn on_browser_created(
      &self,
      browser_view: Option<&mut BrowserView>,
      browser: Option<&mut Browser>,
    ) {
      let (Some(browser_view), Some(browser)) = (browser_view, browser) else {
        return;
      };
      if let Some(on_created) = self.on_created.lock().unwrap().take() {
        let Some(window) = browser_view.window() else {
          log::error!("native Wayland browser view has no CEF window");
          return;
        };
        on_created(
          browser.clone(),
          NativeWindow {
            window,
            browser_view: browser_view.clone(),
            allow_close: self.allow_close.clone(),
          },
        );
      }
    }

    fn on_popup_browser_view_created(
      &self,
      _browser_view: Option<&mut BrowserView>,
      popup_browser_view: Option<&mut BrowserView>,
      _is_devtools: i32,
    ) -> i32 {
      // Sable has one top-level window. A popup policy can redirect navigation;
      // an allowed native popup is closed instead of creating a second window.
      if let Some(browser) = popup_browser_view.and_then(|view| view.browser())
        && let Some(host) = browser.host()
      {
        host.close_browser(1);
      }
      1
    }
  }
}

wrap_window_delegate! {
  struct NativeWindowDelegate {
    browser_view: BrowserView,
    config: WindowConfig,
    allow_close: Arc<AtomicBool>,
  }

  impl ViewDelegate {
    fn preferred_size(&self, _view: Option<&mut View>) -> Size {
      Size {
        width: self.config.bounds.width,
        height: self.config.bounds.height,
      }
    }

    fn minimum_size(&self, _view: Option<&mut View>) -> Size {
      self
        .config
        .min_size
        .map(|size| cef_size_from_tauri(size, self.config.initial_scale_factor))
        .unwrap_or_default()
    }
  }

  impl PanelDelegate {}

  impl WindowDelegate {
    fn on_window_created(&self, window: Option<&mut Window>) {
      let Some(window) = window else { return };
      window.set_to_fill_layout();
      let mut view = View::from(&self.browser_view);
      window.add_child_view(Some(&mut view));
      window.set_title(Some(&CefString::from(self.config.title.as_str())));
      if self.config.visible {
        window.show();
      }
    }

    fn on_window_destroyed(&self, _window: Option<&mut Window>) {
      (self.config.emit)(Event::Destroyed);
    }

    fn on_window_activation_changed(&self, _window: Option<&mut Window>, active: i32) {
      (self.config.emit)(Event::Focused(active != 0));
    }

    fn initial_bounds(&self, _window: Option<&mut Window>) -> Rect {
      self.config.bounds.clone()
    }

    fn initial_show_state(&self, _window: Option<&mut Window>) -> ShowState {
      self.config.show_state
    }

    fn is_frameless(&self, _window: Option<&mut Window>) -> i32 {
      i32::from(self.config.frameless)
    }

    fn can_resize(&self, _window: Option<&mut Window>) -> i32 {
      i32::from(self.config.resizable)
    }

    fn can_maximize(&self, _window: Option<&mut Window>) -> i32 {
      i32::from(self.config.maximizable)
    }

    fn can_minimize(&self, _window: Option<&mut Window>) -> i32 {
      i32::from(self.config.minimizable)
    }

    fn can_close(&self, _window: Option<&mut Window>) -> i32 {
      if self.allow_close.load(Ordering::Acquire) {
        1
      } else if !self.config.closable {
        0
      } else {
        (self.config.emit)(Event::CloseRequested);
        0
      }
    }

    fn window_runtime_style(&self) -> RuntimeStyle {
      RuntimeStyle::CHROME
    }

    fn linux_window_properties(
      &self,
      _window: Option<&mut Window>,
      properties: Option<&mut LinuxWindowProperties>,
    ) -> i32 {
      let Some(properties) = properties else { return 0 };
      properties.wayland_app_id = CefString::from(self.config.app_id.as_str());
      1
    }
  }
}

pub(crate) fn create(
  client: &mut Client,
  url: &CefString,
  settings: &BrowserSettings,
  request_context: Option<&mut RequestContext>,
  config: WindowConfig,
  on_created: BrowserCreated,
) -> Option<()> {
  let allow_close = Arc::new(AtomicBool::new(false));
  let mut browser_delegate =
    NativeBrowserViewDelegate::new(Arc::new(Mutex::new(Some(on_created))), allow_close.clone());
  let browser_view = browser_view_create(
    Some(client),
    Some(url),
    Some(settings),
    None,
    request_context,
    Some(&mut browser_delegate),
  )?;
  let mut window_delegate =
    NativeWindowDelegate::new(browser_view.clone(), config, allow_close.clone());
  window_create_top_level(Some(&mut window_delegate))?;
  Some(())
}
