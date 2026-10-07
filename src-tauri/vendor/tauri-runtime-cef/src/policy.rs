// Copyright 2019-2024 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! Deny-by-default policy for Chromium permission requests and popups.
//!
//! Every privileged Chromium permission reaching CEF — media capture
//! (`getUserMedia`/`getDisplayMedia`) and permission prompts (notifications,
//! geolocation, clipboard, …) — is answered by the process-global policy set
//! with [`set_permission_policy`]. The policy sees a runtime-neutral request
//! ([`PermissionKind`], [`NormalizedOrigin`]) instead of raw CEF bitmasks, and
//! answers through a [`PermissionResponder`] that completes the CEF callback
//! **exactly once**, whatever happens: an immediate verdict, an asynchronous
//! one from a native consent prompt, a timeout, a closing browser, or a
//! responder the policy simply dropped. No path completes with an approval by
//! accident — every failure mode denies.
//!
//! With no policy set, every request is denied ([`DenyReason::NoPolicy`]): an
//! app that never configures a policy grants no privileged Chromium access,
//! rather than silently inheriting the browser's defaults.
//!
//! ## All-or-nothing enforcement
//!
//! The policy evaluates each [`PermissionKind`] separately (see
//! [`PermissionResponder::decide`]), but CEF grants a request as a whole: for
//! `getUserMedia`, `cef_media_access_callback_t::cont` requires the granted
//! mask to *equal* the requested one, and a permission prompt takes a single
//! accept/deny. So a request is granted only when every requested kind is
//! allowed; a partial verdict denies the whole request. Each kind's individual
//! verdict still reaches the audit sink.

use std::{
  collections::HashMap,
  fmt,
  sync::{
    Arc, Mutex, OnceLock, Weak,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  time::Duration,
};

/// How long a deferred (interactive) request may stay unanswered before it is
/// denied — see [`PermissionResponder::defer`].
pub const DEFAULT_PROMPT_TIMEOUT: Duration = Duration::from_secs(30);

/// A privileged browser capability, named independently of CEF's bitmasks.
///
/// Only the permissions Chromium actually routes through CEF's permission
/// handler appear here. Bits this crate does not know become
/// [`PermissionKind::Unknown`], which a policy must deny (there is no way to
/// tell what it would be granting).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PermissionKind {
  Microphone,
  Camera,
  CameraPanTiltZoom,
  /// `getDisplayMedia` — screen/window/tab capture, audio or video.
  ScreenCapture,
  CapturedSurfaceControl,
  /// The async clipboard *read* permission. Clipboard writes are gesture-gated
  /// by Chromium and never reach a permission handler.
  ClipboardRead,
  Geolocation,
  Notifications,
  MidiSysex,
  PointerLock,
  KeyboardLock,
  IdleDetection,
  LocalFonts,
  StorageAccess,
  TopLevelStorageAccess,
  DiskQuota,
  ProtectedMediaIdentifier,
  RegisterProtocolHandler,
  MultipleDownloads,
  WindowManagement,
  FileSystemAccess,
  LocalNetwork,
  Sensors,
  HandTracking,
  IdentityProvider,
  WebAppInstallation,
  /// AR/VR immersive session.
  ImmersiveSession,
  /// A CEF permission bit unknown to this crate — carries the raw bit.
  Unknown(u32),
}

impl fmt::Display for PermissionKind {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Microphone => f.write_str("microphone"),
      Self::Camera => f.write_str("camera"),
      Self::CameraPanTiltZoom => f.write_str("camera-pan-tilt-zoom"),
      Self::ScreenCapture => f.write_str("screen-capture"),
      Self::CapturedSurfaceControl => f.write_str("captured-surface-control"),
      Self::ClipboardRead => f.write_str("clipboard-read"),
      Self::Geolocation => f.write_str("geolocation"),
      Self::Notifications => f.write_str("notifications"),
      Self::MidiSysex => f.write_str("midi-sysex"),
      Self::PointerLock => f.write_str("pointer-lock"),
      Self::KeyboardLock => f.write_str("keyboard-lock"),
      Self::IdleDetection => f.write_str("idle-detection"),
      Self::LocalFonts => f.write_str("local-fonts"),
      Self::StorageAccess => f.write_str("storage-access"),
      Self::TopLevelStorageAccess => f.write_str("top-level-storage-access"),
      Self::DiskQuota => f.write_str("disk-quota"),
      Self::ProtectedMediaIdentifier => f.write_str("protected-media-identifier"),
      Self::RegisterProtocolHandler => f.write_str("register-protocol-handler"),
      Self::MultipleDownloads => f.write_str("multiple-downloads"),
      Self::WindowManagement => f.write_str("window-management"),
      Self::FileSystemAccess => f.write_str("file-system-access"),
      Self::LocalNetwork => f.write_str("local-network"),
      Self::Sensors => f.write_str("sensors"),
      Self::HandTracking => f.write_str("hand-tracking"),
      Self::IdentityProvider => f.write_str("identity-provider"),
      Self::WebAppInstallation => f.write_str("web-app-installation"),
      Self::ImmersiveSession => f.write_str("immersive-session"),
      Self::Unknown(bit) => write!(f, "unknown({bit:#x})"),
    }
  }
}

/// Which CEF callback a request arrived on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestSource {
  /// `OnRequestMediaAccessPermission` — `getUserMedia`/`getDisplayMedia`.
  MediaAccess,
  /// `OnShowPermissionPrompt` — a Chromium permission prompt.
  Prompt,
}

/// A requesting origin, normalized for policy matching: scheme and host
/// lowercased (by [`url`]), the port resolved to the scheme's default when the
/// URL omits it.
///
/// Origins without a host — `null`, opaque, `data:`, `file:`, malformed — do
/// not normalize: [`PermissionRequest::origin`] is [`None`] for those and a
/// policy should deny them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NormalizedOrigin {
  pub scheme: String,
  pub host: String,
  pub port: Option<u16>,
}

impl NormalizedOrigin {
  /// Normalize a serialized origin (`https://example.com:443`). Returns [`None`]
  /// for opaque/host-less/malformed origins.
  pub fn parse(raw: &str) -> Option<Self> {
    let url = url::Url::parse(raw).ok()?;
    let host = url.host_str()?;
    if host.is_empty() {
      return None;
    }
    Some(Self {
      scheme: url.scheme().to_ascii_lowercase(),
      host: host.to_ascii_lowercase(),
      port: url.port_or_known_default(),
    })
  }

  /// Whether this origin is served by the app itself rather than remote web
  /// content: one of the app's registered custom schemes (see
  /// [`crate::CefConfig::custom_schemes`]), or http(s) on a
  /// localhost/loopback/`*.localhost` host.
  ///
  /// This is a *transport* fact, not a trust decision: an app that proxies
  /// remote content through a local origin (a gateway, a preview server) must
  /// not treat app-local as trusted — distinguish those by webview label.
  pub fn is_app_local(&self) -> bool {
    if crate::config::config()
      .custom_schemes
      .iter()
      .any(|scheme| scheme == &self.scheme)
    {
      return true;
    }
    matches!(self.scheme.as_str(), "http" | "https")
      && (self.host == "localhost"
        || self.host == "127.0.0.1"
        || self.host == "[::1]"
        || self.host == "::1"
        || self.host.ends_with(".localhost"))
  }
}

impl fmt::Display for NormalizedOrigin {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}://{}", self.scheme, self.host)?;
    if let Some(port) = self.port {
      write!(f, ":{port}")?;
    }
    Ok(())
  }
}

/// A privileged-permission request awaiting a policy verdict.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
  /// Process-unique id; appears in the matching [`PermissionAudit`].
  pub id: u64,
  /// Label of the tauri webview the request came from.
  pub webview_label: String,
  /// The normalized requesting origin, or [`None`] when the origin is opaque,
  /// host-less or malformed (deny those).
  pub origin: Option<NormalizedOrigin>,
  /// The origin exactly as CEF reported it (`""` when it reported none).
  pub raw_origin: String,
  /// Every permission this request asks for, translated from CEF's bitmask.
  pub kinds: Vec<PermissionKind>,
  /// Whether the requesting frame is the top-level frame. [`None`] when CEF
  /// does not report a frame (permission prompts are browser-scoped).
  pub is_main_frame: Option<bool>,
  pub source: RequestSource,
}

/// A per-kind verdict from the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
  Allow,
  Deny,
}

/// Why a request was denied. Recorded in the [`PermissionAudit`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenyReason {
  /// No policy was configured — the crate default.
  NoPolicy,
  /// The policy denied it.
  PolicyDenied,
  /// The policy asked the user, who said no.
  UserDenied,
  /// The origin was opaque, host-less or malformed.
  InvalidOrigin,
  /// A permission this build cannot reason about.
  UnsupportedPermission,
  /// Not every requested kind was allowed — CEF cannot grant a subset.
  PartialGrantRefused,
  /// A deferred request was not answered within its timeout.
  RequestExpired,
  /// The webview went away while the request was pending.
  BrowserClosed,
  /// The policy dropped the responder without deciding — a policy bug, denied
  /// so it cannot become an accidental grant.
  ProviderDropped,
}

impl fmt::Display for DenyReason {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    let reason = match self {
      Self::NoPolicy => "no-policy",
      Self::PolicyDenied => "policy-denied",
      Self::UserDenied => "user-denied",
      Self::InvalidOrigin => "invalid-origin",
      Self::UnsupportedPermission => "unsupported-permission",
      Self::PartialGrantRefused => "partial-grant-refused",
      Self::RequestExpired => "request-expired",
      Self::BrowserClosed => "browser-closed",
      Self::ProviderDropped => "provider-dropped",
    };
    f.write_str(reason)
  }
}

/// The final, enforced outcome of a request: exactly what CEF was told.
#[derive(Debug, Clone)]
pub struct PermissionAudit {
  pub request_id: u64,
  pub webview_label: String,
  pub origin: Option<NormalizedOrigin>,
  pub raw_origin: String,
  pub kinds: Vec<PermissionKind>,
  /// Per-kind verdicts when the policy decided; empty when it never got that
  /// far (timeout, browser close, no policy).
  pub verdicts: Vec<Verdict>,
  pub granted: bool,
  /// Set whenever `granted` is false.
  pub reason: Option<DenyReason>,
  pub source: RequestSource,
}

/// Shared state of one in-flight request. Completing it is idempotent: the
/// first completion wins and answers CEF, every later one is a no-op.
struct Pending {
  request: PermissionRequest,
  done: AtomicBool,
  complete: Box<dyn Fn(bool) + Send + Sync>,
}

impl Pending {
  /// Answer CEF and emit the audit event. Returns false if already answered.
  fn finish(&self, granted: bool, verdicts: Vec<Verdict>, reason: Option<DenyReason>) -> bool {
    if self
      .done
      .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
      .is_err()
    {
      return false;
    }
    (self.complete)(granted);
    let audit = PermissionAudit {
      request_id: self.request.id,
      webview_label: self.request.webview_label.clone(),
      origin: self.request.origin.clone(),
      raw_origin: self.request.raw_origin.clone(),
      kinds: self.request.kinds.clone(),
      verdicts,
      granted,
      reason,
      source: self.request.source,
    };
    if granted {
      log::info!(
        "granted {:?} to {} in webview {:?}",
        audit.kinds,
        audit.raw_origin,
        audit.webview_label
      );
    } else {
      log::info!(
        "denied {:?} ({}) to {} in webview {:?}",
        audit.kinds,
        reason.map(|r| r.to_string()).unwrap_or_default(),
        audit.raw_origin,
        audit.webview_label
      );
    }
    if let Some(sink) = AUDIT_SINK.get() {
      sink(&audit);
    }
    true
  }
}

/// The policy's handle on one request. It must be consumed — dropping it
/// without a verdict denies the request ([`DenyReason::ProviderDropped`]).
///
/// The CEF callback stays alive behind this handle, so a policy may take the
/// decision off the CEF thread with [`defer`](Self::defer) and answer later.
#[must_use = "dropping a responder without deciding denies the request"]
pub struct PermissionResponder {
  pending: Option<Arc<Pending>>,
}

impl PermissionResponder {
  /// Allow every requested permission.
  pub fn allow(mut self) {
    let pending = self.pending.take().expect("responder consumed once");
    let verdicts = vec![Verdict::Allow; pending.request.kinds.len()];
    pending.finish(true, verdicts, None);
  }

  /// Deny the whole request.
  pub fn deny(mut self, reason: DenyReason) {
    let pending = self.pending.take().expect("responder consumed once");
    let verdicts = vec![Verdict::Deny; pending.request.kinds.len()];
    pending.finish(false, verdicts, Some(reason));
  }

  /// Answer each requested kind separately: `verdicts[i]` is the verdict for
  /// `request.kinds[i]`.
  ///
  /// CEF cannot grant a subset (see the module docs), so the request is
  /// granted only when every verdict is [`Verdict::Allow`]; a mixed verdict
  /// denies the request with [`DenyReason::PartialGrantRefused`] while the
  /// per-kind verdicts still reach the audit sink. A length mismatch is a
  /// policy bug and denies.
  pub fn decide(mut self, verdicts: Vec<Verdict>) {
    let pending = self.pending.take().expect("responder consumed once");
    if verdicts.len() != pending.request.kinds.len() {
      log::error!(
        "permission policy returned {} verdicts for {} kinds — denying",
        verdicts.len(),
        pending.request.kinds.len()
      );
      let denied = vec![Verdict::Deny; pending.request.kinds.len()];
      pending.finish(false, denied, Some(DenyReason::ProviderDropped));
      return;
    }
    match refusal(&verdicts) {
      None => {
        pending.finish(true, verdicts, None);
      }
      Some(reason) => {
        pending.finish(false, verdicts, Some(reason));
      }
    }
  }

  /// Take the decision off the CEF thread — for a native consent prompt.
  ///
  /// The returned handle can be answered from any thread. If it is not
  /// answered within `timeout`, the request is denied
  /// ([`DenyReason::RequestExpired`]); it is also denied if the webview closes
  /// first, or if the handle is dropped.
  pub fn defer(mut self, timeout: Duration) -> DeferredResponder {
    let pending = self.pending.take().expect("responder consumed once");
    register_pending(&pending);
    // ponytail: one timer thread per interactive prompt — prompts are rare and
    // short-lived. Move to a shared timer wheel if that stops being true.
    let timer = Arc::downgrade(&pending);
    std::thread::spawn(move || {
      std::thread::sleep(timeout);
      if let Some(pending) = timer.upgrade() {
        let denied = vec![Verdict::Deny; pending.request.kinds.len()];
        pending.finish(false, denied, Some(DenyReason::RequestExpired));
      }
    });
    DeferredResponder { pending }
  }
}

/// Why a set of per-kind verdicts denies the request, or [`None`] when every
/// kind was allowed and the request is granted whole.
fn refusal(verdicts: &[Verdict]) -> Option<DenyReason> {
  if verdicts.iter().all(|verdict| *verdict == Verdict::Allow) {
    None
  } else if verdicts.contains(&Verdict::Allow) {
    // Some allowed, some not: CEF has no way to grant the subset.
    Some(DenyReason::PartialGrantRefused)
  } else {
    // Nothing was allowed — an ordinary denial, not a subset CEF refused.
    Some(DenyReason::PolicyDenied)
  }
}

impl Drop for PermissionResponder {
  fn drop(&mut self) {
    if let Some(pending) = self.pending.take() {
      log::error!(
        "permission policy dropped request {} without deciding — denying",
        pending.request.id
      );
      let denied = vec![Verdict::Deny; pending.request.kinds.len()];
      pending.finish(false, denied, Some(DenyReason::ProviderDropped));
    }
  }
}

/// A request whose verdict is being decided off the CEF thread (a native
/// consent prompt). Answering it after it has already been resolved — by a
/// timeout, a closing webview, or a previous answer — is a harmless no-op that
/// returns `false`.
pub struct DeferredResponder {
  pending: Arc<Pending>,
}

impl DeferredResponder {
  pub fn request_id(&self) -> u64 {
    self.pending.request.id
  }

  pub fn request(&self) -> &PermissionRequest {
    &self.pending.request
  }

  /// Whether this request is still awaiting an answer.
  pub fn is_live(&self) -> bool {
    !self.pending.done.load(Ordering::SeqCst)
  }

  /// Allow every requested permission. Returns whether this call was the one
  /// that answered CEF.
  pub fn allow(&self) -> bool {
    let verdicts = vec![Verdict::Allow; self.pending.request.kinds.len()];
    self.pending.finish(true, verdicts, None)
  }

  /// Deny the request. Returns whether this call was the one that answered CEF.
  pub fn deny(&self, reason: DenyReason) -> bool {
    let verdicts = vec![Verdict::Deny; self.pending.request.kinds.len()];
    self.pending.finish(false, verdicts, Some(reason))
  }
}

impl Drop for DeferredResponder {
  fn drop(&mut self) {
    if self.is_live() {
      self.deny(DenyReason::ProviderDropped);
    }
  }
}

type PermissionPolicy = dyn Fn(PermissionRequest, PermissionResponder) + Send + Sync;
type AuditSink = dyn Fn(&PermissionAudit) + Send + Sync;

static PERMISSION_POLICY: OnceLock<Box<PermissionPolicy>> = OnceLock::new();
static AUDIT_SINK: OnceLock<Box<AuditSink>> = OnceLock::new();
static NEXT_REQUEST_ID: AtomicU64 = AtomicU64::new(1);
/// In-flight deferred requests, per webview label, so a closing webview can
/// deny them instead of leaving a prompt hanging over a dead browser.
static PENDING: Mutex<Option<HashMap<String, Vec<Weak<Pending>>>>> = Mutex::new(None);

/// Sets the process-global permission policy. Call before `tauri::Builder::run`
/// (later calls are ignored). Without one, every privileged request is denied.
///
/// The policy runs on a CEF thread: it must not block. To ask the user, call
/// [`PermissionResponder::defer`] and answer from the app's event loop.
pub fn set_permission_policy(
  policy: impl Fn(PermissionRequest, PermissionResponder) + Send + Sync + 'static,
) {
  let _ = PERMISSION_POLICY.set(Box::new(policy));
}

/// Sets the sink for permission audit events — one per *final* decision,
/// emitted after CEF has been answered. Call before `tauri::Builder::run`.
pub fn set_permission_audit(sink: impl Fn(&PermissionAudit) + Send + Sync + 'static) {
  let _ = AUDIT_SINK.set(Box::new(sink));
}

fn register_pending(pending: &Arc<Pending>) {
  let mut guard = PENDING.lock().expect("permission registry poisoned");
  let registry = guard.get_or_insert_with(HashMap::new);
  let entries = registry
    .entry(pending.request.webview_label.clone())
    .or_default();
  entries.retain(|entry| {
    entry
      .upgrade()
      .is_some_and(|entry| !entry.done.load(Ordering::SeqCst))
  });
  entries.push(Arc::downgrade(pending));
}

/// Deny every request still pending for `label` — its webview is going away.
pub(crate) fn cancel_pending(label: &str) {
  let entries = {
    let mut guard = PENDING.lock().expect("permission registry poisoned");
    guard.as_mut().and_then(|registry| registry.remove(label))
  };
  for entry in entries.into_iter().flatten() {
    if let Some(pending) = entry.upgrade() {
      let denied = vec![Verdict::Deny; pending.request.kinds.len()];
      pending.finish(false, denied, Some(DenyReason::BrowserClosed));
    }
  }
}

/// Route one CEF permission request through the policy. `complete` answers the
/// CEF callback: `true` grants exactly the requested set, `false` grants
/// nothing.
pub(crate) fn dispatch(
  webview_label: &str,
  raw_origin: &str,
  source: RequestSource,
  kinds: Vec<PermissionKind>,
  is_main_frame: Option<bool>,
  complete: impl Fn(bool) + Send + Sync + 'static,
) {
  let request = PermissionRequest {
    id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
    webview_label: webview_label.to_string(),
    origin: NormalizedOrigin::parse(raw_origin),
    raw_origin: raw_origin.to_string(),
    kinds,
    is_main_frame,
    source,
  };
  let pending = Arc::new(Pending {
    request: request.clone(),
    done: AtomicBool::new(false),
    complete: Box::new(complete),
  });
  let responder = PermissionResponder {
    pending: Some(pending),
  };
  match PERMISSION_POLICY.get() {
    Some(policy) => policy(request, responder),
    None => responder.deny(DenyReason::NoPolicy),
  }
}

/// CEF permission-prompt bits → [`PermissionKind`]s.
pub(crate) fn prompt_kinds(mask: u32) -> Vec<PermissionKind> {
  use cef::sys::cef_permission_request_types_t as bits;
  let table = [
    (
      bits::CEF_PERMISSION_TYPE_AR_SESSION as u32,
      PermissionKind::ImmersiveSession,
    ),
    (
      bits::CEF_PERMISSION_TYPE_VR_SESSION as u32,
      PermissionKind::ImmersiveSession,
    ),
    (
      bits::CEF_PERMISSION_TYPE_CAMERA_PAN_TILT_ZOOM as u32,
      PermissionKind::CameraPanTiltZoom,
    ),
    (
      bits::CEF_PERMISSION_TYPE_CAMERA_STREAM as u32,
      PermissionKind::Camera,
    ),
    (
      bits::CEF_PERMISSION_TYPE_MIC_STREAM as u32,
      PermissionKind::Microphone,
    ),
    (
      bits::CEF_PERMISSION_TYPE_CAPTURED_SURFACE_CONTROL as u32,
      PermissionKind::CapturedSurfaceControl,
    ),
    (
      bits::CEF_PERMISSION_TYPE_CLIPBOARD as u32,
      PermissionKind::ClipboardRead,
    ),
    (
      bits::CEF_PERMISSION_TYPE_TOP_LEVEL_STORAGE_ACCESS as u32,
      PermissionKind::TopLevelStorageAccess,
    ),
    (
      bits::CEF_PERMISSION_TYPE_DISK_QUOTA as u32,
      PermissionKind::DiskQuota,
    ),
    (
      bits::CEF_PERMISSION_TYPE_LOCAL_FONTS as u32,
      PermissionKind::LocalFonts,
    ),
    (
      bits::CEF_PERMISSION_TYPE_GEOLOCATION as u32,
      PermissionKind::Geolocation,
    ),
    (
      bits::CEF_PERMISSION_TYPE_HAND_TRACKING as u32,
      PermissionKind::HandTracking,
    ),
    (
      bits::CEF_PERMISSION_TYPE_IDENTITY_PROVIDER as u32,
      PermissionKind::IdentityProvider,
    ),
    (
      bits::CEF_PERMISSION_TYPE_IDLE_DETECTION as u32,
      PermissionKind::IdleDetection,
    ),
    (
      bits::CEF_PERMISSION_TYPE_MIDI_SYSEX as u32,
      PermissionKind::MidiSysex,
    ),
    (
      bits::CEF_PERMISSION_TYPE_MULTIPLE_DOWNLOADS as u32,
      PermissionKind::MultipleDownloads,
    ),
    (
      bits::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32,
      PermissionKind::Notifications,
    ),
    (
      bits::CEF_PERMISSION_TYPE_KEYBOARD_LOCK as u32,
      PermissionKind::KeyboardLock,
    ),
    (
      bits::CEF_PERMISSION_TYPE_POINTER_LOCK as u32,
      PermissionKind::PointerLock,
    ),
    (
      bits::CEF_PERMISSION_TYPE_PROTECTED_MEDIA_IDENTIFIER as u32,
      PermissionKind::ProtectedMediaIdentifier,
    ),
    (
      bits::CEF_PERMISSION_TYPE_REGISTER_PROTOCOL_HANDLER as u32,
      PermissionKind::RegisterProtocolHandler,
    ),
    (
      bits::CEF_PERMISSION_TYPE_STORAGE_ACCESS as u32,
      PermissionKind::StorageAccess,
    ),
    (
      bits::CEF_PERMISSION_TYPE_WEB_APP_INSTALLATION as u32,
      PermissionKind::WebAppInstallation,
    ),
    (
      bits::CEF_PERMISSION_TYPE_WINDOW_MANAGEMENT as u32,
      PermissionKind::WindowManagement,
    ),
    (
      bits::CEF_PERMISSION_TYPE_FILE_SYSTEM_ACCESS as u32,
      PermissionKind::FileSystemAccess,
    ),
    (
      bits::CEF_PERMISSION_TYPE_LOCAL_NETWORK as u32,
      PermissionKind::LocalNetwork,
    ),
    (
      bits::CEF_PERMISSION_TYPE_LOOPBACK_NETWORK as u32,
      PermissionKind::LocalNetwork,
    ),
    (
      bits::CEF_PERMISSION_TYPE_SENSORS as u32,
      PermissionKind::Sensors,
    ),
  ];
  kinds_from_mask(mask, &table)
}

/// CEF media-access bits → [`PermissionKind`]s. Desktop audio and video are
/// both part of the one screen-capture permission decision.
pub(crate) fn media_kinds(mask: u32) -> Vec<PermissionKind> {
  use cef::sys::cef_media_access_permission_types_t as bits;
  let table = [
    (
      bits::CEF_MEDIA_PERMISSION_DEVICE_AUDIO_CAPTURE as u32,
      PermissionKind::Microphone,
    ),
    (
      bits::CEF_MEDIA_PERMISSION_DEVICE_VIDEO_CAPTURE as u32,
      PermissionKind::Camera,
    ),
    (
      bits::CEF_MEDIA_PERMISSION_DESKTOP_AUDIO_CAPTURE as u32,
      PermissionKind::ScreenCapture,
    ),
    (
      bits::CEF_MEDIA_PERMISSION_DESKTOP_VIDEO_CAPTURE as u32,
      PermissionKind::ScreenCapture,
    ),
  ];
  kinds_from_mask(mask, &table)
}

/// Translate a bitmask, deduplicating kinds and turning every bit the table
/// does not name into [`PermissionKind::Unknown`] — so a policy denying
/// unknown kinds keeps denying permissions added by a future Chromium.
fn kinds_from_mask(mask: u32, table: &[(u32, PermissionKind)]) -> Vec<PermissionKind> {
  let mut kinds: Vec<PermissionKind> = Vec::new();
  let mut push = |kind: PermissionKind| {
    if !kinds.contains(&kind) {
      kinds.push(kind);
    }
  };
  let known: u32 = table
    .iter()
    .map(|(bit, _)| *bit)
    .fold(0, |acc, bit| acc | bit);
  for (bit, kind) in table {
    if mask & bit != 0 {
      push(*kind);
    }
  }
  let mut unknown = mask & !known;
  while unknown != 0 {
    let bit = unknown & unknown.wrapping_neg();
    push(PermissionKind::Unknown(bit));
    unknown &= !bit;
  }
  kinds
}

/// A `window.open` / popup request routed through the policy set by
/// [`set_popup_policy`].
#[derive(Debug)]
pub struct PopupRequest<'a> {
  /// Label of the tauri webview that initiated the popup.
  pub webview_label: &'a str,
  /// The URL the popup wants to open. Empty if CEF reports none.
  pub url: &'a str,
}

type PopupPolicy = dyn Fn(&PopupRequest<'_>) -> bool + Send + Sync;

static POPUP_POLICY: OnceLock<Box<PopupPolicy>> = OnceLock::new();

/// Sets the process-global popup policy (`true` = allow the popup as a
/// native CEF window). When unset, webviews whose tauri builder installed an
/// `on_new_window` handler deny popups and all others allow them — see
/// `life_span.rs` for why the tauri handler itself cannot be consulted.
pub fn set_popup_policy(policy: impl Fn(&PopupRequest<'_>) -> bool + Send + Sync + 'static) {
  let _ = POPUP_POLICY.set(Box::new(policy));
}

pub(crate) fn popup_allowed(request: &PopupRequest<'_>) -> Option<bool> {
  POPUP_POLICY.get().map(|policy| policy(request))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::sync::mpsc;

  /// A request wired to a recording sink instead of a CEF callback: `outcomes`
  /// receives one `bool` per completion that actually reached CEF.
  fn pending(kinds: Vec<PermissionKind>) -> (PermissionResponder, mpsc::Receiver<bool>) {
    let (tx, rx) = mpsc::channel();
    let request = PermissionRequest {
      id: NEXT_REQUEST_ID.fetch_add(1, Ordering::Relaxed),
      webview_label: "test".into(),
      origin: NormalizedOrigin::parse("https://example.com"),
      raw_origin: "https://example.com".into(),
      kinds,
      is_main_frame: Some(true),
      source: RequestSource::MediaAccess,
    };
    let responder = PermissionResponder {
      pending: Some(Arc::new(Pending {
        request,
        done: AtomicBool::new(false),
        complete: Box::new(move |granted| {
          let _ = tx.send(granted);
        }),
      })),
    };
    (responder, rx)
  }

  #[test]
  fn allow_grants_and_deny_refuses() {
    let (responder, granted) = pending(vec![PermissionKind::Microphone]);
    responder.allow();
    assert_eq!(granted.try_recv(), Ok(true));

    let (responder, granted) = pending(vec![PermissionKind::Camera]);
    responder.deny(DenyReason::PolicyDenied);
    assert_eq!(granted.try_recv(), Ok(false));
  }

  #[test]
  fn a_dropped_responder_denies() {
    let (responder, granted) = pending(vec![PermissionKind::Camera]);
    drop(responder);
    assert_eq!(
      granted.try_recv(),
      Ok(false),
      "a policy that drops the responder must not grant"
    );
  }

  #[test]
  fn a_partial_verdict_denies_the_whole_request() {
    let (responder, granted) = pending(vec![PermissionKind::Microphone, PermissionKind::Camera]);
    responder.decide(vec![Verdict::Allow, Verdict::Deny]);
    assert_eq!(
      granted.try_recv(),
      Ok(false),
      "CEF cannot grant a subset — a mixed verdict denies"
    );

    let (responder, granted) = pending(vec![PermissionKind::Microphone, PermissionKind::Camera]);
    responder.decide(vec![Verdict::Allow, Verdict::Allow]);
    assert_eq!(granted.try_recv(), Ok(true));
  }

  #[test]
  fn the_audit_reason_tells_a_plain_no_from_a_refused_subset() {
    assert_eq!(refusal(&[Verdict::Allow, Verdict::Allow]), None);
    assert_eq!(
      refusal(&[Verdict::Deny, Verdict::Deny]),
      Some(DenyReason::PolicyDenied),
      "nothing was allowed — an ordinary denial, not a subset CEF refused"
    );
    assert_eq!(
      refusal(&[Verdict::Allow, Verdict::Deny]),
      Some(DenyReason::PartialGrantRefused)
    );
  }

  #[test]
  fn a_verdict_of_the_wrong_length_denies() {
    let (responder, granted) = pending(vec![PermissionKind::Microphone, PermissionKind::Camera]);
    responder.decide(vec![Verdict::Allow]);
    assert_eq!(granted.try_recv(), Ok(false));
  }

  #[test]
  fn cef_is_answered_exactly_once() {
    let (responder, granted) = pending(vec![PermissionKind::Microphone]);
    let deferred = responder.defer(Duration::from_secs(60));
    assert!(deferred.allow(), "first answer wins");
    assert!(!deferred.deny(DenyReason::UserDenied), "second is a no-op");
    assert!(!deferred.allow());
    drop(deferred);
    assert_eq!(granted.try_recv(), Ok(true));
    assert!(
      granted.try_recv().is_err(),
      "CEF must be answered exactly once"
    );
  }

  #[test]
  fn a_deferred_request_times_out_into_a_denial() {
    let (responder, granted) = pending(vec![PermissionKind::Camera]);
    let deferred = responder.defer(Duration::from_millis(50));
    assert_eq!(
      granted.recv_timeout(Duration::from_secs(5)),
      Ok(false),
      "an unanswered prompt must deny"
    );
    assert!(!deferred.is_live());
    assert!(!deferred.allow(), "a timed-out request cannot be granted");
  }

  #[test]
  fn a_dropped_deferred_responder_denies() {
    let (responder, granted) = pending(vec![PermissionKind::Camera]);
    drop(responder.defer(Duration::from_secs(60)));
    assert_eq!(granted.try_recv(), Ok(false));
  }

  #[test]
  fn a_closing_webview_denies_its_pending_requests() {
    let (responder, granted) = pending(vec![PermissionKind::Camera]);
    let deferred = responder.defer(Duration::from_secs(60));
    cancel_pending("test");
    assert_eq!(granted.try_recv(), Ok(false));
    assert!(!deferred.is_live());
    assert!(!deferred.allow(), "a closed browser cannot be granted to");
  }

  #[test]
  fn origins_normalize_and_opaque_ones_do_not() {
    let origin = NormalizedOrigin::parse("HTTPS://Example.COM").expect("normalizes");
    assert_eq!(origin.scheme, "https");
    assert_eq!(origin.host, "example.com");
    assert_eq!(origin.port, Some(443), "default port is resolved");
    assert_eq!(origin.to_string(), "https://example.com:443");

    assert_ne!(
      NormalizedOrigin::parse("https://example.com"),
      NormalizedOrigin::parse("http://example.com"),
      "scheme distinguishes origins on the same host"
    );

    for opaque in [
      "null",
      "",
      "about:blank",
      "data:text/html,x",
      "file:///etc/passwd",
      "not a url",
    ] {
      assert!(
        NormalizedOrigin::parse(opaque).is_none(),
        "{opaque} must not normalize"
      );
    }
  }

  #[test]
  fn permission_masks_translate_and_unknown_bits_stay_unknown() {
    use cef::sys::cef_media_access_permission_types_t as media;
    use cef::sys::cef_permission_request_types_t as prompt;
    assert_eq!(
      media_kinds(
        media::CEF_MEDIA_PERMISSION_DEVICE_AUDIO_CAPTURE as u32
          | media::CEF_MEDIA_PERMISSION_DEVICE_VIDEO_CAPTURE as u32
      ),
      vec![PermissionKind::Microphone, PermissionKind::Camera]
    );
    assert_eq!(
      media_kinds(
        media::CEF_MEDIA_PERMISSION_DESKTOP_AUDIO_CAPTURE as u32
          | media::CEF_MEDIA_PERMISSION_DESKTOP_VIDEO_CAPTURE as u32
      ),
      vec![PermissionKind::ScreenCapture]
    );

    assert_eq!(
      prompt_kinds(prompt::CEF_PERMISSION_TYPE_GEOLOCATION as u32),
      vec![PermissionKind::Geolocation]
    );
    assert_eq!(
      prompt_kinds(prompt::CEF_PERMISSION_TYPE_CLIPBOARD as u32),
      vec![PermissionKind::ClipboardRead]
    );

    let future_bit = 1u32 << 31;
    assert_eq!(
      prompt_kinds(prompt::CEF_PERMISSION_TYPE_NOTIFICATIONS as u32 | future_bit),
      vec![
        PermissionKind::Notifications,
        PermissionKind::Unknown(future_bit)
      ],
      "a permission this build does not know must surface as Unknown, not vanish"
    );
  }
}
