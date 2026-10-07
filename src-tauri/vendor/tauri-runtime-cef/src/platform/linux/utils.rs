// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  cell::RefCell,
  ffi::CString,
  os::raw::{c_int, c_long, c_ulong},
  sync::LazyLock,
};
use x11_dl::xlib;

const NET_WM_STATE_REMOVE: c_long = 0;
const NET_WM_STATE_ADD: c_long = 1;
const CLIENT_MESSAGE: i32 = 33;
const SUBSTRUCTURE_REDIRECT_MASK: c_long = 1 << 20;
const SUBSTRUCTURE_NOTIFY_MASK: c_long = 1 << 19;

static XLIB: LazyLock<Option<xlib::Xlib>> = LazyLock::new(|| xlib::Xlib::open().ok());

struct Display(*mut xlib::Display);

thread_local! {
  static DISPLAY: RefCell<Option<Display>> = const { RefCell::new(None) };
}

pub(super) fn with_cef_display<R>(
  default: R,
  f: impl FnOnce(&xlib::Xlib, *mut xlib::Display) -> R,
) -> R {
  let Some(xlib) = XLIB.as_ref() else {
    return default;
  };
  let display = cef::get_xdisplay() as *mut xlib::Display;
  if display.is_null() {
    return default;
  }

  let result = f(xlib, display);
  unsafe {
    (xlib.XFlush)(display);
  }
  result
}

pub(super) fn with_x11<R>(default: R, f: impl FnOnce(&xlib::Xlib, *mut xlib::Display) -> R) -> R {
  let Some(xlib) = XLIB.as_ref() else {
    return default;
  };

  DISPLAY.with(|cell| {
    let mut guard = cell.borrow_mut();
    if guard.is_none() {
      let display = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
      if display.is_null() {
        return default;
      }
      *guard = Some(Display(display));
    }

    let display = guard.as_ref().unwrap().0;
    let result = f(xlib, display);
    unsafe {
      (xlib.XFlush)(display);
    }
    result
  })
}

unsafe extern "C" fn x_error_handler(
  _display: *mut xlib::Display,
  event: *mut xlib::XErrorEvent,
) -> c_int {
  if !event.is_null() {
    let event = unsafe { &*event };
    log::warn!(
      "X error received: type {}, serial {}, error_code {}, request_code {}, minor_code {}",
      event.type_,
      event.serial,
      event.error_code,
      event.request_code,
      event.minor_code
    );
  }
  0
}

unsafe extern "C" fn x_io_error_handler(_display: *mut xlib::Display) -> c_int {
  log::error!("X IO error received: the display connection is gone");
  0
}

/// Replace Xlib's process-killing default error handlers with logging no-ops.
///
/// Xlib terminates the process on error by default: the stock error handler
/// prints and calls `exit(1)`, and the IO-error handler exits when the display
/// connection breaks. A non-fatal X protocol error — the kind a compositor or a
/// GPU reset produces on display resume — therefore takes the whole app down
/// with no Rust panic and no backtrace.
///
/// Mirrors cefclient's `XErrorHandlerImpl`/`XIOErrorHandlerImpl`, installed
/// there for the same reason:
/// <https://github.com/chromiumembedded/cef/blob/master/tests/cefclient/cefclient_gtk.cc>
///
/// The runtime installs these itself after `cef::initialize`. **An embedder
/// that calls `gtk_init` must call this again afterwards**: GTK's X11 backend
/// installs its own handler during init, replacing whatever was there. This is
/// why cefclient installs its handlers *after* `gtk_init` rather than before.
/// Calling this more than once is harmless.
pub fn install_x_error_handlers() {
  #[cfg(target_os = "linux")]
  if crate::config::native_wayland() {
    return;
  }

  let Some(xlib) = XLIB.as_ref() else {
    return;
  };

  unsafe {
    (xlib.XSetErrorHandler)(Some(x_error_handler));
    (xlib.XSetIOErrorHandler)(Some(x_io_error_handler));
  }
}

pub(super) fn atom(xlib: &xlib::Xlib, display: *mut xlib::Display, name: &str) -> c_ulong {
  let cname = CString::new(name).unwrap();
  unsafe { (xlib.XInternAtom)(display, cname.as_ptr(), 0) }
}

pub(super) fn set_wm_state(xid: c_ulong, add: bool, atom1: &str, atom2: Option<&str>) {
  with_x11((), |xlib, display| {
    let wm_state = atom(xlib, display, "_NET_WM_STATE");
    let a1 = atom(xlib, display, atom1);
    let a2 = atom2.map(|name| atom(xlib, display, name)).unwrap_or(0);
    let action = if add {
      NET_WM_STATE_ADD
    } else {
      NET_WM_STATE_REMOVE
    };

    unsafe {
      let root = (xlib.XDefaultRootWindow)(display);
      let mut event: xlib::XEvent = std::mem::zeroed();
      event.client_message = xlib::XClientMessageEvent {
        type_: CLIENT_MESSAGE,
        serial: 0,
        send_event: 1,
        display,
        window: xid,
        message_type: wm_state,
        format: 32,
        data: xlib::ClientMessageData::from([action, a1 as c_long, a2 as c_long, 1, 0]),
      };
      (xlib.XSendEvent)(
        display,
        root,
        0,
        SUBSTRUCTURE_REDIRECT_MASK | SUBSTRUCTURE_NOTIFY_MASK,
        &mut event,
      );
    }
  });
}
