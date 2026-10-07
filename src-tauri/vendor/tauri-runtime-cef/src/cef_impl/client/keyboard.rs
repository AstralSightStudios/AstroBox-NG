// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use cef::*;

fn is_browser_shortcut(key_code: i32, primary_modifier: bool, alt: bool) -> bool {
  (primary_modifier
    && matches!(
      key_code,
      9 | 33 | 34 | 49..=57 | 72 | 74 | 76 | 78 | 79 | 80 | 82 | 83 | 84 | 87
    ))
    || (alt && matches!(key_code, 37 | 39))
    || matches!(key_code, 116 | 117 | 122)
}

#[cfg(target_os = "linux")]
type CefOsEvent<'a> = Option<&'a mut cef::sys::XEvent>;
#[cfg(target_os = "macos")]
type CefOsEvent<'a> = *mut u8;
#[cfg(windows)]
type CefOsEvent<'a> = Option<&'a mut cef::sys::MSG>;

wrap_keyboard_handler! {
  pub struct TauriCefKeyboardHandler {
    devtools_enabled: bool,
  }

  impl KeyboardHandler {
    fn on_pre_key_event(
      &self,
      _browser: Option<&mut Browser>,
      event: Option<&KeyEvent>,
      _os_event: CefOsEvent<'_>,
      is_keyboard_shortcut: Option<&mut ::std::os::raw::c_int>,
    ) -> ::std::os::raw::c_int {
      let Some(event) = event else {
        return 0;
      };

      // Check if this is a keydown event.
      use cef::sys::cef_key_event_type_t;
      let keydown_type: cef::KeyEventType = cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into();
      if event.type_ != keydown_type {
        return 0;
      }

      // Get modifier keys.
      use cef::sys::cef_event_flags_t;
      #[cfg(windows)]
      let modifiers = event.modifiers as i32;
      #[cfg(not(windows))]
      let modifiers = event.modifiers;

      #[cfg(not(target_os = "macos"))]
      let primary_modifier = (modifiers & (cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0)) != 0;
      #[cfg(target_os = "macos")]
      let primary_modifier = (modifiers & (cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0)) != 0;
      let alt = (modifiers & (cef_event_flags_t::EVENTFLAG_ALT_DOWN.0)) != 0;
      #[cfg(not(target_os = "macos"))]
      let shift = (modifiers & (cef_event_flags_t::EVENTFLAG_SHIFT_DOWN.0)) != 0;

      let key_code = event.windows_key_code;

      if is_browser_shortcut(key_code, primary_modifier, alt) {
        if let Some(is_keyboard_shortcut) = is_keyboard_shortcut {
          *is_keyboard_shortcut = 1;
        }
        return 0;
      }

      // If devtools is disabled, block devtools keyboard shortcuts.
      if !self.devtools_enabled {
        // Block F12 (key code 123).
        if key_code == 123 {
          if let Some(is_keyboard_shortcut) = is_keyboard_shortcut {
            *is_keyboard_shortcut = 1;
          }
          return 1;
        }

        // Block Ctrl+Shift+I on Linux/Windows and Cmd+Opt+I on macOS.
        #[cfg(not(target_os = "macos"))]
        let devtools_shortcut = key_code == 73 && primary_modifier && shift;
        #[cfg(target_os = "macos")]
        let devtools_shortcut = key_code == 73 && primary_modifier && alt;
        if devtools_shortcut {
          if let Some(is_keyboard_shortcut) = is_keyboard_shortcut {
            *is_keyboard_shortcut = 1;
          }
          return 1;
        }
      }

      0
    }

    fn on_key_event(
      &self,
      _browser: Option<&mut Browser>,
      event: Option<&KeyEvent>,
      _os_event: CefOsEvent<'_>,
    ) -> ::std::os::raw::c_int {
      let Some(event) = event else {
        return 0;
      };

      use cef::sys::{cef_event_flags_t, cef_key_event_type_t};
      let keydown_type: cef::KeyEventType = cef_key_event_type_t::KEYEVENT_RAWKEYDOWN.into();
      if event.type_ != keydown_type {
        return 0;
      }

      #[cfg(windows)]
      let modifiers = event.modifiers as i32;
      #[cfg(not(windows))]
      let modifiers = event.modifiers;

      #[cfg(not(target_os = "macos"))]
      let primary_modifier = (modifiers & (cef_event_flags_t::EVENTFLAG_CONTROL_DOWN.0)) != 0;
      #[cfg(target_os = "macos")]
      let primary_modifier = (modifiers & (cef_event_flags_t::EVENTFLAG_COMMAND_DOWN.0)) != 0;
      let alt = (modifiers & (cef_event_flags_t::EVENTFLAG_ALT_DOWN.0)) != 0;

      is_browser_shortcut(event.windows_key_code, primary_modifier, alt) as ::std::os::raw::c_int
    }
  }
}

#[cfg(test)]
mod tests {
  use super::is_browser_shortcut;

  #[test]
  fn blocks_chromium_browser_accelerators() {
    assert!(is_browser_shortcut(84, true, false));
    assert!(is_browser_shortcut(9, true, false));
    assert!(is_browser_shortcut(37, false, true));
    assert!(is_browser_shortcut(116, false, false));
  }

  #[test]
  fn allows_webview_shortcuts() {
    assert!(!is_browser_shortcut(66, true, false));
    assert!(!is_browser_shortcut(70, true, false));
    assert!(!is_browser_shortcut(73, true, false));
  }
}
