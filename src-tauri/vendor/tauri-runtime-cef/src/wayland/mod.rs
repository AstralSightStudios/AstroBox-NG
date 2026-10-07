//! Native Wayland app backed by a CEF Views top-level window.

mod webview;
mod window;

use std::{
  sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
    mpsc::{self, Receiver, Sender},
  },
  time::{Duration, Instant},
};

use cef::ImplBrowserHost;
use raw_window_handle::HasDisplayHandle;
use tauri_runtime::{
  DeviceEventFilter, Error, ExitRequestedEventAction, Result, RunEvent, UserEvent,
  dpi::PhysicalSize,
  window::{PendingWindow, WindowEvent, WindowId},
};
use winit::{
  application::ApplicationHandler,
  event::StartCause,
  event_loop::{ActiveEventLoop, ControlFlow},
};

use crate::{
  cef_impl::request_handler,
  runtime::{AfterWindowCreationCallback, CefRuntime, EventLoopMessage, Message, RuntimeContext},
  webview::AppWebview,
  window::WindowMessage,
  window_handle::SendRawDisplayHandle,
};

use window::{WaylandWindow, WindowConfig};

const CEF_WORK_INTERVAL: Duration = Duration::from_millis(4);

pub(crate) struct App<T: UserEvent> {
  context: RuntimeContext<T>,
  receiver: Receiver<Message<T>>,
  native_event_sender: Sender<(WindowId, window::Event)>,
  native_event_receiver: Receiver<(WindowId, window::Event)>,
  window: Option<WaylandWindow>,
  callback: Box<dyn FnMut(RunEvent<T>)>,
  scheme_registry: request_handler::SchemeRegistry,
  browser_live: bool,
  exiting: bool,
  exit_code: Arc<AtomicI32>,
}

impl<T: UserEvent> App<T> {
  pub(crate) fn new(
    context: RuntimeContext<T>,
    receiver: Receiver<Message<T>>,
    callback: Box<dyn FnMut(RunEvent<T>)>,
    scheme_registry: request_handler::SchemeRegistry,
    exit_code: Arc<AtomicI32>,
  ) -> Self {
    let (native_event_sender, native_event_receiver) = mpsc::channel();
    Self {
      context,
      receiver,
      native_event_sender,
      native_event_receiver,
      window: None,
      callback,
      scheme_registry,
      browser_live: false,
      exiting: false,
      exit_code,
    }
  }

  fn install_dispatch(
    &mut self,
    event_loop: &dyn ActiveEventLoop,
  ) -> crate::runtime::MainThreadDispatchGuard<T> {
    unsafe fn handle<T: UserEvent>(
      app: *mut (),
      event_loop: &dyn ActiveEventLoop,
      message: Message<T>,
    ) {
      unsafe { &mut *app.cast::<App<T>>() }.handle_message(event_loop, message);
    }
    let app = (self as *mut Self).cast();
    self
      .context
      .install_current_dispatch(app, handle::<T>, event_loop)
  }

  fn run_callback(&mut self, event: RunEvent<T>) {
    (self.callback)(event);
  }

  fn drain_messages(&mut self, event_loop: &dyn ActiveEventLoop) {
    while let Ok(message) = self.receiver.try_recv() {
      self.handle_message(event_loop, message);
    }
    while let Ok((window_id, event)) = self.native_event_receiver.try_recv() {
      self.handle_window_event(event_loop, window_id, event);
    }
  }

  fn handle_message(&mut self, event_loop: &dyn ActiveEventLoop, message: Message<T>) {
    match message {
      Message::EventLoop(message) => self.handle_event_loop_message(event_loop, message),
      Message::BrowserClosed(..) => {
        let child = self.window.as_mut().and_then(|window| window.child.take());
        if let Some(child) = child {
          self.remove_scheme_entries(&child);
        }
        self.browser_live = false;
        self.exit_if_done(event_loop);
      }
      Message::CreateWindow {
        window_id,
        webview_id,
        pending,
        after_window_creation,
        result_tx,
      } => {
        let result = self.create_window(
          event_loop,
          window_id,
          webview_id,
          *pending,
          after_window_creation,
        );
        _ = result_tx.send(result);
      }
      Message::CreateWebview {
        window_id: _,
        webview_id: _,
        pending: _,
        result_tx,
      } => {
        _ = result_tx.send(Err(Error::CreateWebview(
          "native Wayland only supports the webview created with its window".into(),
        )))
      }
      Message::Window { window_id, message } => {
        self.handle_window_message(event_loop, window_id, message)
      }
      Message::Webview {
        window_id,
        webview_id: _,
        message,
      } => {
        if !self.exiting
          && let Some(window) = self.window.as_mut().filter(|window| window.id == window_id)
          && window.child.is_some()
        {
          webview::handle_message(window_id, window, message);
        }
      }
      Message::DragDropScriptEvent { .. } => {}
      Message::Task(task) => task(),
      Message::RequestExit(code) => {
        if self.request_exit(Some(code)) {
          self.exit_code.store(code, Ordering::Release);
          match self.window.as_ref().map(|window| window.id) {
            Some(window_id) => self.close_window(window_id, event_loop, true),
            None => self.exit_if_done(event_loop),
          }
        }
      }
      Message::Opened(urls) => log::warn!(
        "dropping deep-link open event {urls:?}: published tauri-runtime has no Linux RunEvent::Opened"
      ),
      Message::UserEvent(event) => self.run_callback(RunEvent::UserEvent(event)),
    }
  }

  fn create_window(
    &mut self,
    event_loop: &dyn ActiveEventLoop,
    window_id: WindowId,
    webview_id: Option<u32>,
    pending: PendingWindow<T, CefRuntime<T>>,
    after_window_creation: Option<AfterWindowCreationCallback>,
  ) -> Result<()> {
    if self.window.is_some() || after_window_creation.is_some() {
      return Err(Error::CreateWindow);
    }
    let (Some(webview_id), Some(pending_webview)) = (webview_id, pending.webview) else {
      return Err(Error::CreateWindow);
    };

    let attrs = pending.window_builder.attrs.clone();
    let scale = event_loop
      .primary_monitor()
      .map(|monitor| monitor.scale_factor())
      .unwrap_or(1.0);
    let mut size = attrs
      .inner
      .surface_size
      .unwrap_or_else(|| PhysicalSize::new(800, 600).into())
      .to_physical::<u32>(scale);
    if let Some(min) = attrs.inner.min_surface_size {
      let min = min.to_physical::<u32>(scale);
      size.width = size.width.max(min.width);
      size.height = size.height.max(min.height);
    }
    if let Some(max) = attrs.inner.max_surface_size {
      let max = max.to_physical::<u32>(scale);
      size.width = size.width.min(max.width);
      size.height = size.height.min(max.height);
    }

    let mut window = WaylandWindow {
      id: window_id,
      label: pending.label,
      attrs,
      child: None,
      listeners: Default::default(),
      native: None,
      initial_surface_size: size,
      initial_scale_factor: scale,
      reported_focus: false,
    };
    let sender = self.native_event_sender.clone();
    let proxy = self.context.proxy.clone();
    let config = WindowConfig::new(
      &window.attrs,
      window.initial_surface_size,
      window.initial_scale_factor,
      move |event| {
        if sender.send((window_id, event)).is_ok() {
          proxy.wake_up();
        }
      },
    );
    // Theme is always resolved as system: nothing sets `preferred_theme` or
    // calls `set_theme` in practice, so this never diverges from ColorVariant::SYSTEM.
    let Some((child, native)) = webview::build(
      &self.context,
      &self.scheme_registry,
      window.id,
      webview_id,
      config,
      None,
      pending_webview,
    ) else {
      return Err(Error::CreateWebview(
        "failed to create CEF Views browser".into(),
      ));
    };
    window.native = Some(native);
    window.child = Some(child);
    self.browser_live = true;
    self.window = Some(window);
    Ok(())
  }

  fn handle_window_message(
    &mut self,
    event_loop: &dyn ActiveEventLoop,
    window_id: WindowId,
    message: WindowMessage,
  ) {
    match message {
      WindowMessage::Close => return self.request_window_close(window_id, event_loop),
      WindowMessage::Destroy => return self.close_window(window_id, event_loop, true),
      _ => {}
    }
    if let Some(window) = self.window.as_mut().filter(|window| window.id == window_id) {
      window::handle_window_message(window, event_loop, message);
    }
  }

  fn handle_window_event(
    &mut self,
    event_loop: &dyn ActiveEventLoop,
    window_id: WindowId,
    event: window::Event,
  ) {
    match event {
      window::Event::CloseRequested => self.request_window_close(window_id, event_loop),
      window::Event::Destroyed => self.close_window(window_id, event_loop, false),
      window::Event::Focused(focused) => {
        let Some(window) = self.window.as_mut().filter(|window| window.id == window_id) else {
          return;
        };
        if window.reported_focus == focused {
          return;
        }
        window.reported_focus = focused;
        log::debug!("native-wayland focus changed: window={window_id:?} focused={focused}");
        if let Some(child) = &window.child {
          child.host.set_focus(i32::from(focused));
          if focused {
            webview::take_focus(child);
          }
        }
        self.emit_window_event(window_id, WindowEvent::Focused(focused));
      }
    }
  }

  fn close_window(
    &mut self,
    window_id: WindowId,
    event_loop: &dyn ActiveEventLoop,
    close_native: bool,
  ) {
    if self
      .window
      .as_ref()
      .is_none_or(|window| window.id != window_id)
    {
      return;
    }
    if !self.exiting {
      self.emit_window_event(window_id, WindowEvent::Destroyed);
    }
    let Some(window) = self.window.take() else {
      return;
    };
    if let Some(child) = &window.child {
      self.remove_scheme_entries(child);
      child.host.close_browser(1);
    }
    if close_native {
      window.close();
    }
    self.exit_if_done(event_loop);
  }

  fn request_window_close(&mut self, window_id: WindowId, event_loop: &dyn ActiveEventLoop) {
    if self.exiting {
      return self.close_window(window_id, event_loop, true);
    }
    let Some(window) = self.window.as_ref().filter(|window| window.id == window_id) else {
      return;
    };
    let label = window.label.clone();
    let listeners = window.listeners.clone();
    let (tx, rx) = mpsc::channel();
    for listener in listeners.lock().unwrap().values() {
      listener(&WindowEvent::CloseRequested {
        signal_tx: tx.clone(),
      });
    }
    self.run_callback(RunEvent::WindowEvent {
      label,
      event: WindowEvent::CloseRequested { signal_tx: tx },
    });
    if !matches!(rx.try_recv(), Ok(true)) {
      self.close_window(window_id, event_loop, true);
    }
  }

  fn remove_scheme_entries(&self, child: &AppWebview) {
    let mut registry = self.scheme_registry.lock().unwrap();
    for scheme in child.uri_scheme_protocols.keys() {
      registry.remove(&(child.browser_id, scheme.clone()));
    }
  }

  fn emit_window_event(&mut self, window_id: WindowId, event: WindowEvent) {
    let Some(window) = self.window.as_ref().filter(|window| window.id == window_id) else {
      return;
    };
    let label = window.label.clone();
    let listeners = window.listeners.clone();
    self.run_callback(RunEvent::WindowEvent {
      label,
      event: event.clone(),
    });
    for listener in listeners.lock().unwrap().values() {
      listener(&event);
    }
  }

  fn request_exit(&mut self, code: Option<i32>) -> bool {
    if self.exiting {
      return false;
    }
    let (tx, rx) = mpsc::channel();
    self.run_callback(RunEvent::ExitRequested { code, tx });
    if matches!(rx.try_recv(), Ok(ExitRequestedEventAction::Prevent)) {
      false
    } else {
      self.exiting = true;
      true
    }
  }

  fn exit_if_done(&mut self, event_loop: &dyn ActiveEventLoop) {
    if self.browser_live {
      return;
    }
    if self.exiting || (self.window.is_none() && self.request_exit(None)) {
      self.run_callback(RunEvent::Exit);
      event_loop.exit();
    }
  }

  fn handle_event_loop_message(
    &mut self,
    event_loop: &dyn ActiveEventLoop,
    message: EventLoopMessage,
  ) {
    match message {
      EventLoopMessage::SetTheme(_) => {}
      EventLoopMessage::SetDeviceEventFilter(filter) => {
        event_loop.listen_device_events(match filter {
          DeviceEventFilter::Always => winit::event_loop::DeviceEvents::Never,
          DeviceEventFilter::Unfocused => winit::event_loop::DeviceEvents::WhenFocused,
          DeviceEventFilter::Never => winit::event_loop::DeviceEvents::Always,
        });
      }
      EventLoopMessage::PrimaryMonitor(tx) => _ = tx.send(None),
      EventLoopMessage::MonitorFromPoint(tx, ..) => _ = tx.send(None),
      EventLoopMessage::AvailableMonitors(tx) => _ = tx.send(Vec::new()),
      EventLoopMessage::CursorPosition(tx) => _ = tx.send(Err(Error::FailedToGetCursorPosition)),
      EventLoopMessage::DisplayHandle(tx) => {
        _ = tx.send(
          event_loop
            .display_handle()
            .map(|handle| SendRawDisplayHandle(handle.as_raw())),
        )
      }
    }
  }

  fn service_cef(&self, event_loop: &dyn ActiveEventLoop) {
    let context = gtk::glib::MainContext::default();
    while context.pending() {
      context.iteration(false);
    }
    cef::do_message_loop_work();
    event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + CEF_WORK_INTERVAL));
  }
}

impl<T: UserEvent> ApplicationHandler for App<T> {
  fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
    let _guard = self.install_dispatch(event_loop);
    match cause {
      StartCause::Init => {
        self.run_callback(RunEvent::Ready);
        self.context.cef_pump.do_work();
      }
      StartCause::Poll => self.run_callback(RunEvent::Resumed),
      _ => {}
    }
  }

  fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
    let _guard = self.install_dispatch(event_loop);
    self.drain_messages(event_loop);
  }

  fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
    let _guard = self.install_dispatch(event_loop);
    self.drain_messages(event_loop);
  }

  fn window_event(
    &mut self,
    _event_loop: &dyn ActiveEventLoop,
    _window_id: winit::window::WindowId,
    _event: winit::event::WindowEvent,
  ) {
  }

  fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
    let _guard = self.install_dispatch(event_loop);
    self.service_cef(event_loop);
    self.run_callback(RunEvent::MainEventsCleared);
  }
}
