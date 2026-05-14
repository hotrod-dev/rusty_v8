//! Round-trip test for embedder-supplied internal-field serialize/deserialize
//! callbacks on v8::SnapshotCreator + v8::Context::new.
//!
//! Before the fast-test-fork rusty_v8 change, the binding.cc hard-coded a
//! dummy 4-byte `InternalFieldData` struct as the (de)serialize callback,
//! which corrupted any aligned pointer stored in an object's internal field
//! across a snapshot/restore cycle. This test verifies that when an embedder
//! supplies its own callbacks, a pointer round-trips bit-for-bit.

use std::ffi::c_void;

const TAG: u16 = 0; // v8::kEmbedderDataTypeTagDefault

// Use the address of a real static so the value satisfies V8's
// aligned-pointer requirement (min 2-byte alignment).
static STORED_INT: u32 = 0x1234_5678;

unsafe extern "C" fn serialize_cb(
  holder: *const v8::Object,
  index: i32,
  _data: *mut c_void,
) -> v8::InternalFieldsPayload {
  let holder = unsafe { &*holder };
  let ptr = unsafe { holder.get_aligned_pointer_from_internal_field(index, TAG) };
  let value = ptr as usize as u64;
  v8::InternalFieldsPayload::from_vec(value.to_le_bytes().to_vec())
}

unsafe extern "C" fn deserialize_cb(
  holder: *const v8::Object,
  index: i32,
  payload: *const u8,
  len: usize,
  _data: *mut c_void,
) {
  assert_eq!(len, 8);
  let holder = unsafe { &*holder };
  let slice = unsafe { std::slice::from_raw_parts(payload, len) };
  let mut buf = [0u8; 8];
  buf.copy_from_slice(slice);
  let value = u64::from_le_bytes(buf) as usize as *const c_void;
  holder.set_aligned_pointer_in_internal_field(index, value, TAG);
}

#[test]
fn internal_field_round_trip() {
  let platform = v8::new_default_platform(0, false).make_shared();
  v8::V8::initialize_platform(platform);
  v8::V8::initialize();

  let expected_ptr = &STORED_INT as *const u32 as *const c_void;

  let blob = {
    let mut isolate = v8::Isolate::snapshot_creator(
      None,
      Some(v8::CreateParams::default()),
    );
    {
      v8::scope!(let scope, &mut isolate);
      let context = v8::Context::new(scope, Default::default());
      {
        let scope = &mut v8::ContextScope::new(scope, context);

        let object_templ = v8::ObjectTemplate::new(scope);
        object_templ.set_internal_field_count(1);
        let object = object_templ.new_instance(scope).unwrap();
        assert_eq!(object.internal_field_count(), 1);
        object.set_aligned_pointer_in_internal_field(0, expected_ptr, TAG);

        let name = v8::String::new(scope, "g").unwrap();
        let global = context.global(scope);
        global.set(scope, name.into(), object.into()).unwrap();
      }
      scope.set_default_context_with_serializer(
        context,
        Some(v8::SerializeInternalFieldsCallback {
          callback: serialize_cb,
          data: std::ptr::null_mut(),
        }),
      );
    }
    isolate.create_blob(v8::FunctionCodeHandling::Keep).unwrap()
  };

  {
    let mut isolate = v8::Isolate::new(
      v8::CreateParams::default().snapshot_blob(blob),
    );
    v8::scope!(let scope, &mut isolate);

    let context = v8::Context::new(
      scope,
      v8::ContextOptions {
        deserialize_internal_fields: Some(
          v8::DeserializeInternalFieldsCallback {
            callback: deserialize_cb,
            data: std::ptr::null_mut(),
          },
        ),
        ..Default::default()
      },
    );
    let scope = &mut v8::ContextScope::new(scope, context);

    let global = context.global(scope);
    let name = v8::String::new(scope, "g").unwrap();
    let value = global.get(scope, name.into()).unwrap();
    let obj = value.to_object(scope).unwrap();
    assert_eq!(obj.internal_field_count(), 1, "internal field survived");

    let restored_ptr = unsafe {
      obj.get_aligned_pointer_from_internal_field(0, TAG)
    };
    assert_eq!(
      restored_ptr, expected_ptr,
      "pointer must round-trip bit-for-bit across the snapshot",
    );
  }

  unsafe {
    v8::V8::dispose();
  }
  v8::V8::dispose_platform();
}
