// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Adapter from CEF's permission prompts to the runtime-neutral policy in
//! [`crate::policy`].
//!
//! Media-access requests use the same policy as Chromium permission prompts.
//!
//! Grants are also written back as content settings and DevTools permission
//! overrides. `OnRequestMediaAccessPermission` bypasses Chromium's permission
//! manager, so otherwise `enumerateDevices` still sees "not granted": it hides
//! device labels and reports one placeholder per kind. Content settings cannot
//! hold a portless custom-scheme origin such as `tauri://localhost`.

use std::sync::{
  Arc, Mutex, PoisonError,
  atomic::{AtomicI32, Ordering},
};

use cef::{rc::Rc as _, *};

use crate::policy::{self, PermissionKind, RequestSource};

static NEXT_OVERRIDE_DEVTOOLS_MESSAGE_ID: AtomicI32 = AtomicI32::new(2_000_000);

wrap_permission_handler! {
  pub struct TauriCefPermissionHandler {
    webview_label: String,
  }

  impl PermissionHandler {
    fn on_request_media_access_permission(
      &self,
      browser: Option<&mut Browser>,
      frame: Option<&mut Frame>,
      requesting_origin: Option<&CefString>,
      requested_permissions: u32,
      callback: Option<&mut MediaAccessCallback>,
    ) -> ::std::os::raw::c_int {
      use cef::sys::cef_media_access_permission_types_t as bits;

      let Some(callback) = callback else {
        return 0;
      };
      let callback = callback.clone();
      let origin = requesting_origin.map(|origin| origin.to_string()).unwrap_or_default();
      let is_main_frame = frame.map(|frame| frame.is_main() != 0);
      let kinds = policy::media_kinds(requested_permissions);

      let host = browser.and_then(|browser| browser.host());
      let request_context = host.as_ref().and_then(|host| host.request_context());
      let content_types = media_content_types(&kinds);

      // A stored grant answers without asking the policy again.
      if let Some(request_context) = request_context.as_ref()
        && !content_types.is_empty()
        && content_types.len() == kinds.len()
        && content_types.iter().all(|content_type| {
          is_allowed(request_context, &origin, *content_type)
        })
      {
        callback.cont(requested_permissions);
        return 1;
      }

      let permission_override = PermissionOverride {
        host,
        origin: origin.clone(),
        names: override_names(&kinds),
      };
      let grant_recorder = GrantRecorder {
        request_context,
        origin: origin.clone(),
        content_types,
      };

      policy::dispatch(
        &self.webview_label,
        &origin,
        RequestSource::MediaAccess,
        kinds,
        is_main_frame,
        move |granted| {
          if granted {
            grant_recorder.record();
            permission_override.apply();
          }
          callback.cont(if granted {
            requested_permissions
          } else {
            bits::CEF_MEDIA_PERMISSION_NONE as u32
          });
        },
      );
      1
    }

    fn on_show_permission_prompt(
      &self,
      _browser: Option<&mut Browser>,
      _prompt_id: u64,
      requesting_origin: Option<&CefString>,
      requested_permissions: u32,
      callback: Option<&mut PermissionPromptCallback>,
    ) -> ::std::os::raw::c_int {
      let Some(callback) = callback else {
        return 0;
      };
      let callback = callback.clone();
      let origin = requesting_origin.map(|origin| origin.to_string()).unwrap_or_default();
      policy::dispatch(
        &self.webview_label,
        &origin,
        RequestSource::Prompt,
        policy::prompt_kinds(requested_permissions),
        // CEF reports no frame for permission prompts — they are browser-scoped.
        None,
        move |granted| {
          let result = if granted {
            cef::sys::cef_permission_request_result_t::CEF_PERMISSION_RESULT_ACCEPT
          } else {
            cef::sys::cef_permission_request_result_t::CEF_PERMISSION_RESULT_DENY
          };
          callback.cont(PermissionRequestResult::from(result));
        },
      );
      1
    }
  }
}

/// `getDisplayMedia` is left out: Chromium asks per use, so nothing persists.
fn media_content_types(kinds: &[PermissionKind]) -> Vec<ContentSettingTypes> {
  kinds
    .iter()
    .filter_map(|kind| match kind {
      PermissionKind::Microphone => Some(ContentSettingTypes::MEDIASTREAM_MIC),
      PermissionKind::Camera => Some(ContentSettingTypes::MEDIASTREAM_CAMERA),
      _ => None,
    })
    .collect()
}

fn override_names(kinds: &[PermissionKind]) -> Vec<&'static str> {
  kinds
    .iter()
    .filter_map(|kind| match kind {
      PermissionKind::Microphone => Some("microphone"),
      PermissionKind::Camera => Some("camera"),
      _ => None,
    })
    .collect()
}

fn is_allowed(
  request_context: &RequestContext,
  origin: &str,
  content_type: ContentSettingTypes,
) -> bool {
  let origin = CefString::from(origin);
  request_context.content_setting(Some(&origin), Some(&origin), content_type)
    == ContentSettingValues::ALLOW
}

/// `SetContentSetting` is UI-thread only; the policy may answer from any thread.
struct GrantRecorder {
  request_context: Option<RequestContext>,
  origin: String,
  content_types: Vec<ContentSettingTypes>,
}

impl GrantRecorder {
  fn record(&self) {
    let Some(request_context) = self.request_context.as_ref() else {
      return;
    };
    if self.content_types.is_empty() || self.origin.is_empty() {
      return;
    }

    if cef::currently_on(cef::sys::cef_thread_id_t::TID_UI.into()) != 0 {
      record_grant(request_context, &self.origin, &self.content_types);
      return;
    }

    let mut task = RecordGrantTask::new(
      request_context.clone(),
      self.origin.clone(),
      self.content_types.clone(),
    );
    cef::post_task(cef::sys::cef_thread_id_t::TID_UI.into(), Some(&mut task));
  }
}

fn record_grant(
  request_context: &RequestContext,
  origin: &str,
  content_types: &[ContentSettingTypes],
) {
  let origin = CefString::from(origin);
  for content_type in content_types {
    request_context.set_content_setting(
      Some(&origin),
      Some(&origin),
      *content_type,
      ContentSettingValues::ALLOW,
    );
  }
}

wrap_task! {
  struct RecordGrantTask {
    request_context: RequestContext,
    origin: String,
    content_types: Vec<ContentSettingTypes>,
  }

  impl Task {
    fn execute(&self) {
      record_grant(&self.request_context, &self.origin, &self.content_types);
    }
  }
}

struct PermissionOverride {
  host: Option<BrowserHost>,
  origin: String,
  names: Vec<&'static str>,
}

impl PermissionOverride {
  fn apply(&self) {
    let Some(host) = self.host.as_ref() else {
      return;
    };
    if self.names.is_empty() || self.origin.is_empty() {
      return;
    }

    if cef::currently_on(cef::sys::cef_thread_id_t::TID_UI.into()) != 0 {
      apply_override(host, &self.origin, &self.names);
      return;
    }

    let mut task = ApplyOverrideTask::new(host.clone(), self.origin.clone(), self.names.clone());
    cef::post_task(cef::sys::cef_thread_id_t::TID_UI.into(), Some(&mut task));
  }
}

fn next_override_message_id() -> i32 {
  NEXT_OVERRIDE_DEVTOOLS_MESSAGE_ID.fetch_add(1, Ordering::Relaxed)
}

fn apply_override(host: &BrowserHost, origin: &str, names: &[&'static str]) {
  let ids: Vec<i32> = names.iter().map(|_| next_override_message_id()).collect();
  let pending = Arc::new(Mutex::new(ids.clone()));
  let registration = Arc::new(Mutex::new(None));
  let mut observer =
    PermissionOverrideObserver::new(origin.to_owned(), pending.clone(), registration.clone());
  let Some(observer_registration) = host.add_dev_tools_message_observer(Some(&mut observer)) else {
    log::warn!("could not observe DevTools to override media permissions for {origin}");
    return;
  };
  *registration.lock().unwrap_or_else(PoisonError::into_inner) = Some(observer_registration);

  for (id, name) in ids.into_iter().zip(names) {
    let message = serde_json::json!({
      "id": id,
      "method": "Browser.setPermission",
      "params": {
        "permission": { "name": name },
        "setting": "granted",
        "origin": origin,
      },
    })
    .to_string();
    if host.send_dev_tools_message(Some(message.as_bytes())) == 1 {
      continue;
    }
    log::warn!("could not send the {name} permission override for {origin}");
    settle(&pending, &registration, id);
  }
}

fn settle(pending: &Mutex<Vec<i32>>, registration: &Mutex<Option<Registration>>, id: i32) -> bool {
  let mut pending = pending.lock().unwrap_or_else(PoisonError::into_inner);
  let Some(index) = pending.iter().position(|pending_id| *pending_id == id) else {
    return false;
  };
  pending.swap_remove(index);
  if pending.is_empty() {
    drop(pending);
    let _ = registration
      .lock()
      .unwrap_or_else(PoisonError::into_inner)
      .take();
  }
  true
}

wrap_dev_tools_message_observer! {
  struct PermissionOverrideObserver {
    origin: String,
    pending: Arc<Mutex<Vec<i32>>>,
    registration: Arc<Mutex<Option<Registration>>>,
  }

  impl DevToolsMessageObserver {
    fn on_dev_tools_method_result(
      &self,
      _browser: Option<&mut Browser>,
      message_id: ::std::os::raw::c_int,
      success: ::std::os::raw::c_int,
      result: Option<&[u8]>,
    ) {
      if settle(&self.pending, &self.registration, message_id) && success == 0 {
        log::warn!(
          "media permission override for {} failed: {}",
          self.origin,
          String::from_utf8_lossy(result.unwrap_or_default())
        );
      }
    }
  }
}

wrap_task! {
  struct ApplyOverrideTask {
    host: BrowserHost,
    origin: String,
    names: Vec<&'static str>,
  }

  impl Task {
    fn execute(&self) {
      apply_override(&self.host, &self.origin, &self.names);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn overrides_only_device_capture() {
    assert_eq!(
      override_names(&[
        PermissionKind::Microphone,
        PermissionKind::Camera,
        PermissionKind::ScreenCapture,
      ]),
      ["microphone", "camera"]
    );
  }
}
