use crate::Context;
use crate::Data;
use crate::Local;
use crate::Object;
use crate::OwnedIsolate;
use crate::external_references::ExternalReference;
use crate::isolate::RealIsolate;
use crate::isolate_create_params::raw;
use crate::support::char;
use crate::support::int;

use std::borrow::Cow;
use std::ffi::c_void;
use std::mem::MaybeUninit;
use std::ptr::null_mut;

unsafe extern "C" {
  fn v8__SnapshotCreator__CONSTRUCT(
    buf: *mut MaybeUninit<SnapshotCreator>,
    params: *const raw::CreateParams,
  );
  fn v8__SnapshotCreator__DESTRUCT(this: *mut SnapshotCreator);
  fn v8__SnapshotCreator__GetIsolate(
    this: *const SnapshotCreator,
  ) -> *mut RealIsolate;
  fn v8__SnapshotCreator__CreateBlob(
    this: *mut SnapshotCreator,
    function_code_handling: FunctionCodeHandling,
  ) -> RawStartupData;
  fn v8__SnapshotCreator__SetDefaultContext(
    this: *mut SnapshotCreator,
    context: *const Context,
    serialize_cb: Option<RawSerializeInternalFieldsFn>,
    serialize_data: *mut c_void,
  );
  fn v8__SnapshotCreator__AddContext(
    this: *mut SnapshotCreator,
    context: *const Context,
    serialize_cb: Option<RawSerializeInternalFieldsFn>,
    serialize_data: *mut c_void,
  ) -> usize;
  fn v8__SnapshotCreator__AddData_to_isolate(
    this: *mut SnapshotCreator,
    data: *const Data,
  ) -> usize;
  fn v8__SnapshotCreator__AddData_to_context(
    this: *mut SnapshotCreator,
    context: *const Context,
    data: *const Data,
  ) -> usize;
  fn v8__StartupData__CanBeRehashed(this: *const RawStartupData) -> bool;
  fn v8__StartupData__IsValid(this: *const RawStartupData) -> bool;
  fn v8__StartupData__data__DELETE(this: *const char);
}

/// Payload returned by a [`SerializeInternalFieldsCallback`]. The pointer must
/// reference a heap allocation that Rust owns — the C++ trampoline copies the
/// bytes into V8-owned storage and then calls [`__internal_field_payload_drop`]
/// to return ownership to Rust for deallocation.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct InternalFieldsPayload {
  pub data: *const u8,
  pub len: usize,
}

impl InternalFieldsPayload {
  pub const EMPTY: Self = Self {
    data: std::ptr::null(),
    len: 0,
  };

  /// Take ownership of `buf`, leaking it into a raw payload. The consumer (the
  /// serialize trampoline) is responsible for calling
  /// [`__internal_field_payload_drop`] with the returned pointer and length.
  pub fn from_vec(buf: Vec<u8>) -> Self {
    if buf.is_empty() {
      return Self::EMPTY;
    }
    let slice: Box<[u8]> = buf.into_boxed_slice();
    let len = slice.len();
    let data = Box::into_raw(slice) as *const u8;
    Self { data, len }
  }
}

/// Raw C ABI for the serialize callback. Implementers should prefer the
/// safe-ish [`SerializeInternalFieldsCallback`] wrapper which handles bridge
/// construction and data ownership.
pub type RawSerializeInternalFieldsFn = unsafe extern "C" fn(
  holder: *const Object,
  index: i32,
  data: *mut c_void,
) -> InternalFieldsPayload;

/// Raw C ABI for the deserialize callback.
pub type RawDeserializeInternalFieldsFn = unsafe extern "C" fn(
  holder: *const Object,
  index: i32,
  payload: *const u8,
  len: usize,
  data: *mut c_void,
);

/// Callback + opaque data pointer pair used when building a snapshot to
/// serialize embedder-owned internal fields on wrapper objects. Pass to
/// [`OwnedIsolate::set_default_context_with_serializer`] or
/// [`OwnedIsolate::add_context_with_serializer`].
#[derive(Copy, Clone, Debug)]
pub struct SerializeInternalFieldsCallback {
  pub callback: RawSerializeInternalFieldsFn,
  pub data: *mut c_void,
}

/// Callback + opaque data pointer pair used when restoring a context from a
/// snapshot to deserialize the internal-field payloads that were written by a
/// matching [`SerializeInternalFieldsCallback`]. Pass via
/// [`ContextOptions::deserialize_internal_fields`].
#[derive(Copy, Clone, Debug)]
pub struct DeserializeInternalFieldsCallback {
  pub callback: RawDeserializeInternalFieldsFn,
  pub data: *mut c_void,
}

/// Called by the C++ serialize trampoline after it has copied the payload
/// bytes into V8-owned storage, to return ownership of the original buffer to
/// Rust so it can be deallocated.
///
/// # Safety
///
/// `ptr` must have come from [`InternalFieldsPayload::from_vec`]; `len` must
/// match.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn rusty_v8__internal_field_payload_drop(
  ptr: *const u8,
  len: usize,
) {
  if ptr.is_null() || len == 0 {
    return;
  }
  unsafe {
    let slice = std::slice::from_raw_parts_mut(ptr as *mut u8, len);
    drop(Box::from_raw(slice as *mut [u8]));
  }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub(crate) struct RawStartupData {
  pub(crate) data: *const char,
  pub(crate) raw_size: int,
}

#[derive(Debug, Clone)]
pub struct StartupData(StartupDataInner);

#[derive(Debug)]
enum StartupDataInner {
  Cpp(RawStartupData),
  Cow(Cow<'static, [u8]>),
}

impl StartupData {
  /// Whether the data created can be rehashed and and the hash seed can be
  /// recomputed when deserialized.
  /// Only valid for StartupData returned by SnapshotCreator::CreateBlob().
  pub fn can_be_rehashed(&self) -> bool {
    let tmp = match &self.0 {
      StartupDataInner::Cpp(t) => *t,
      StartupDataInner::Cow(c) => RawStartupData {
        data: c.as_ptr() as _,
        raw_size: c.len() as _,
      },
    };
    unsafe { v8__StartupData__CanBeRehashed(&tmp) }
  }

  /// Allows embedders to verify whether the data is valid for the current
  /// V8 instance.
  pub fn is_valid(&self) -> bool {
    let tmp = match &self.0 {
      StartupDataInner::Cpp(t) => *t,
      StartupDataInner::Cow(c) => RawStartupData {
        data: c.as_ptr() as _,
        raw_size: c.len() as _,
      },
    };
    unsafe { v8__StartupData__IsValid(&tmp) }
  }
}

impl std::ops::Deref for StartupData {
  type Target = [u8];

  fn deref(&self) -> &Self::Target {
    match &self.0 {
      StartupDataInner::Cpp(t) => unsafe {
        std::slice::from_raw_parts(t.data as _, t.raw_size as _)
      },
      StartupDataInner::Cow(c) => c,
    }
  }
}

impl<T> From<T> for StartupData
where
  T: Into<Cow<'static, [u8]>>,
{
  fn from(value: T) -> Self {
    Self(StartupDataInner::Cow(value.into()))
  }
}

impl Drop for StartupData {
  fn drop(&mut self) {
    if let StartupDataInner::Cpp(raw) = self.0 {
      unsafe {
        v8__StartupData__data__DELETE(raw.data);
      }
    }
  }
}

impl Clone for StartupDataInner {
  fn clone(&self) -> Self {
    match self {
      // Cpp -> Cow to maintain unique ownership of the underlying pointer
      Self::Cpp(r) => Self::Cow(
        unsafe { std::slice::from_raw_parts(r.data as _, r.raw_size as _) }
          .to_vec()
          .into(),
      ),
      Self::Cow(c) => Self::Cow(c.clone()),
    }
  }
}

#[repr(C)]
#[derive(Debug)]
pub enum FunctionCodeHandling {
  Clear,
  Keep,
}

/// Helper class to create a snapshot data blob.
#[repr(C)]
#[derive(Debug)]
pub(crate) struct SnapshotCreator([usize; 1]);

impl SnapshotCreator {
  /// Create an isolate, and set it up for serialization.
  /// The isolate is created from scratch.
  #[inline(always)]
  #[allow(clippy::new_ret_no_self)]
  pub(crate) fn new(
    external_references: Option<Cow<'static, [ExternalReference]>>,
    params: Option<crate::CreateParams>,
  ) -> OwnedIsolate {
    Self::new_impl(external_references, None, params)
  }

  /// Create an isolate, and set it up for serialization.
  /// The isolate is created from scratch.
  #[inline(always)]
  #[allow(clippy::new_ret_no_self)]
  pub(crate) fn from_existing_snapshot(
    existing_snapshot_blob: StartupData,
    external_references: Option<Cow<'static, [ExternalReference]>>,
    params: Option<crate::CreateParams>,
  ) -> OwnedIsolate {
    Self::new_impl(external_references, Some(existing_snapshot_blob), params)
  }

  /// Create and enter an isolate, and set it up for serialization.
  /// The isolate is created from scratch.
  #[inline(always)]
  #[allow(clippy::new_ret_no_self)]
  fn new_impl(
    external_references: Option<Cow<'static, [ExternalReference]>>,
    existing_snapshot_blob: Option<StartupData>,
    params: Option<crate::CreateParams>,
  ) -> OwnedIsolate {
    let mut snapshot_creator: MaybeUninit<Self> = MaybeUninit::uninit();

    let mut params = params.unwrap_or_default();
    if let Some(external_refs) = external_references {
      params = params.external_references(external_refs);
    }
    if let Some(snapshot_blob) = existing_snapshot_blob {
      params = params.snapshot_blob(snapshot_blob);
    }
    let (raw_create_params, create_param_allocations) = params.finalize();

    let snapshot_creator = unsafe {
      v8__SnapshotCreator__CONSTRUCT(&mut snapshot_creator, &raw_create_params);
      snapshot_creator.assume_init()
    };

    let isolate_ptr =
      unsafe { v8__SnapshotCreator__GetIsolate(&snapshot_creator) };
    let mut owned_isolate = OwnedIsolate::new_already_entered(isolate_ptr);
    owned_isolate.initialize(create_param_allocations);
    owned_isolate.set_snapshot_creator(snapshot_creator);
    owned_isolate
  }
}

impl Drop for SnapshotCreator {
  fn drop(&mut self) {
    unsafe { v8__SnapshotCreator__DESTRUCT(self) };
  }
}

impl SnapshotCreator {
  /// Set the default context to be included in the snapshot blob.
  /// The snapshot will not contain the global proxy, and we expect one or a
  /// global object template to create one, to be provided upon deserialization.
  #[inline(always)]
  pub(crate) fn set_default_context(
    &mut self,
    context: Local<Context>,
    serializer: Option<SerializeInternalFieldsCallback>,
  ) {
    let (cb, data) = match serializer {
      Some(s) => (Some(s.callback), s.data),
      None => (None, null_mut()),
    };
    unsafe {
      v8__SnapshotCreator__SetDefaultContext(self, &*context, cb, data)
    };
  }

  /// Add additional context to be included in the snapshot blob.
  /// The snapshot will include the global proxy.
  ///
  /// Returns the index of the context in the snapshot blob.
  #[inline(always)]
  pub(crate) fn add_context(
    &mut self,
    context: Local<Context>,
    serializer: Option<SerializeInternalFieldsCallback>,
  ) -> usize {
    let (cb, data) = match serializer {
      Some(s) => (Some(s.callback), s.data),
      None => (None, null_mut()),
    };
    unsafe { v8__SnapshotCreator__AddContext(self, &*context, cb, data) }
  }

  /// Attach arbitrary `v8::Data` to the isolate snapshot, which can be
  /// retrieved via `HandleScope::get_context_data_from_snapshot_once()` after
  /// deserialization. This data does not survive when a new snapshot is created
  /// from an existing snapshot.
  #[inline(always)]
  pub(crate) fn add_isolate_data<T>(&mut self, data: Local<T>) -> usize
  where
    for<'l> Local<'l, T>: Into<Local<'l, Data>>,
  {
    unsafe { v8__SnapshotCreator__AddData_to_isolate(self, &*data.into()) }
  }

  /// Attach arbitrary `v8::Data` to the context snapshot, which can be
  /// retrieved via `HandleScope::get_context_data_from_snapshot_once()` after
  /// deserialization. This data does not survive when a new snapshot is
  /// created from an existing snapshot.
  #[inline(always)]
  pub(crate) fn add_context_data<T>(
    &mut self,
    context: Local<Context>,
    data: Local<T>,
  ) -> usize
  where
    for<'l> Local<'l, T>: Into<Local<'l, Data>>,
  {
    unsafe {
      v8__SnapshotCreator__AddData_to_context(self, &*context, &*data.into())
    }
  }

  /// Creates a snapshot data blob.
  /// This must not be called from within a handle scope.
  #[inline(always)]
  pub(crate) fn create_blob(
    &mut self,
    function_code_handling: FunctionCodeHandling,
  ) -> Option<StartupData> {
    let blob =
      unsafe { v8__SnapshotCreator__CreateBlob(self, function_code_handling) };
    if blob.data.is_null() {
      debug_assert!(blob.raw_size == 0);
      None
    } else {
      debug_assert!(blob.raw_size > 0);
      Some(StartupData(StartupDataInner::Cpp(blob)))
    }
  }
}
