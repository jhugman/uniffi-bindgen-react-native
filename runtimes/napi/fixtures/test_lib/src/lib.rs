use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Mutex;

#[repr(C)]
pub struct RustBuffer {
    pub capacity: u64,
    pub len: u64,
    pub data: *mut u8,
}

#[repr(C)]
pub struct ForeignBytes {
    pub len: i32,
    pub data: *const u8,
}

#[repr(C)]
pub struct RustCallStatus {
    pub code: i8,
    pub error_buf: RustBuffer,
}

// --- Scalar functions ---

#[no_mangle]
pub extern "C" fn uniffi_test_fn_add(a: i32, b: i32, status: &mut RustCallStatus) -> i32 {
    status.code = 0;
    a + b
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_negate(x: i8, status: &mut RustCallStatus) -> i8 {
    status.code = 0;
    -x
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_handle(status: &mut RustCallStatus) -> u64 {
    status.code = 0;
    0xDEAD_BEEF_1234_5678u64
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_void(status: &mut RustCallStatus) {
    status.code = 0;
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_double(x: f64, status: &mut RustCallStatus) -> f64 {
    status.code = 0;
    x * 2.0
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_float32_half(x: f32, status: &mut RustCallStatus) -> f32 {
    status.code = 0;
    x / 2.0
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_i16_negate(x: i16, status: &mut RustCallStatus) -> i16 {
    status.code = 0;
    -x
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_u16_double(x: u16, status: &mut RustCallStatus) -> u16 {
    status.code = 0;
    x * 2
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_i64_negate(x: i64, status: &mut RustCallStatus) -> i64 {
    status.code = 0;
    -x
}

// --- RustBuffer helpers ---

fn free_buffer(buf: RustBuffer) {
    if !buf.data.is_null() && buf.capacity > 0 {
        let layout = std::alloc::Layout::from_size_align(buf.capacity as usize, 1).unwrap();
        unsafe { std::alloc::dealloc(buf.data, layout) };
    }
}

fn alloc_rustbuffer(data: &[u8]) -> RustBuffer {
    let len = data.len();
    if len == 0 {
        return RustBuffer {
            capacity: 0,
            len: 0,
            data: ptr::null_mut(),
        };
    }
    let layout = std::alloc::Layout::from_size_align(len, 1).unwrap();
    let ptr = unsafe {
        let ptr = std::alloc::alloc(layout);
        ptr::copy_nonoverlapping(data.as_ptr(), ptr, len);
        ptr
    };
    RustBuffer {
        capacity: len as u64,
        len: len as u64,
        data: ptr,
    }
}

fn new_cb_status() -> RustCallStatus {
    RustCallStatus {
        code: 0,
        error_buf: RustBuffer {
            capacity: 0,
            len: 0,
            data: ptr::null_mut(),
        },
    }
}

// --- RustBuffer management ---

#[no_mangle]
pub extern "C" fn uniffi_test_rustbuffer_alloc(
    size: u64,
    status: &mut RustCallStatus,
) -> RustBuffer {
    status.code = 0;
    let layout = std::alloc::Layout::from_size_align(size as usize, 1).unwrap();
    let data = unsafe { std::alloc::alloc_zeroed(layout) };
    RustBuffer {
        capacity: size,
        len: 0,
        data,
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_rustbuffer_free(buf: RustBuffer, status: &mut RustCallStatus) {
    status.code = 0;
    free_buffer(buf);
}

#[no_mangle]
pub extern "C" fn uniffi_test_rustbuffer_from_bytes(
    bytes: ForeignBytes,
    status: &mut RustCallStatus,
) -> RustBuffer {
    status.code = 0;
    if bytes.len == 0 || bytes.data.is_null() {
        return RustBuffer {
            capacity: 0,
            len: 0,
            data: ptr::null_mut(),
        };
    }
    let slice = unsafe { std::slice::from_raw_parts(bytes.data, bytes.len as usize) };
    alloc_rustbuffer(slice)
}

// --- RustBuffer echo (takes buffer, returns same bytes) ---

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_buffer(
    buf: RustBuffer,
    status: &mut RustCallStatus,
) -> RustBuffer {
    status.code = 0;
    let len = buf.len as usize;
    if len == 0 || buf.data.is_null() {
        return RustBuffer {
            capacity: 0,
            len: 0,
            data: ptr::null_mut(),
        };
    }
    let slice = unsafe { std::slice::from_raw_parts(buf.data, len) };
    let new_buf = alloc_rustbuffer(slice);
    free_buffer(buf);
    new_buf
}

// --- RustBuffer multi-arg and utility functions ---

#[no_mangle]
pub extern "C" fn uniffi_test_fn_concat_buffers(
    buf1: RustBuffer,
    buf2: RustBuffer,
    status: &mut RustCallStatus,
) -> RustBuffer {
    status.code = 0;
    let len1 = buf1.len as usize;
    let len2 = buf2.len as usize;

    // Collect bytes from both buffers, then free originals
    let mut combined = Vec::with_capacity(len1 + len2);
    if len1 > 0 && !buf1.data.is_null() {
        combined.extend_from_slice(unsafe { std::slice::from_raw_parts(buf1.data, len1) });
    }
    if len2 > 0 && !buf2.data.is_null() {
        combined.extend_from_slice(unsafe { std::slice::from_raw_parts(buf2.data, len2) });
    }
    free_buffer(buf1);
    free_buffer(buf2);

    alloc_rustbuffer(&combined)
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_buffer_len(buf: RustBuffer, status: &mut RustCallStatus) -> u32 {
    status.code = 0;
    let len = buf.len as u32;
    free_buffer(buf);
    len
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_make_buffer(
    value: u8,
    count: u32,
    status: &mut RustCallStatus,
) -> RustBuffer {
    status.code = 0;
    let data = vec![value; count as usize];
    alloc_rustbuffer(&data)
}

// --- Error-producing function ---

#[no_mangle]
pub extern "C" fn uniffi_test_fn_error(status: &mut RustCallStatus) -> i32 {
    status.code = 2; // CALL_UNEXPECTED_ERROR
    status.error_buf = alloc_rustbuffer(b"something went wrong");
    0
}

// --- Callback test: calls a function pointer ---

pub type SimpleCallback = extern "C" fn(u64, i8);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_call_callback(
    cb: SimpleCallback,
    handle: u64,
    value: i8,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    cb(handle, value);
}

// --- Callback with RustBuffer arg ---

pub type BufferCallback = extern "C" fn(u64, RustBuffer);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_call_callback_with_buffer(
    cb: BufferCallback,
    handle: u64,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    // Callback takes ownership of the buffer — do NOT free here
    cb(handle, alloc_rustbuffer(&[0xDE, 0xAD, 0xBE, 0xEF]));
}

// --- Callback from another thread ---

#[no_mangle]
pub extern "C" fn uniffi_test_fn_call_callback_from_thread(
    cb: SimpleCallback,
    handle: u64,
    value: i8,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cb(handle, value);
    });
}

// --- VTable test ---

#[repr(C)]
pub struct TestVTable {
    pub get_value: extern "C" fn(u64, &mut RustCallStatus) -> i32,
    pub free: extern "C" fn(u64, &mut RustCallStatus),
}

static STORED_VTABLE: Mutex<Option<TestVTable>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_init_vtable(vtable: &TestVTable, status: &mut RustCallStatus) {
    status.code = 0;
    *STORED_VTABLE.lock().unwrap() = Some(TestVTable {
        get_value: vtable.get_value,
        free: vtable.free,
    });
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_use_vtable(handle: u64, status: &mut RustCallStatus) -> i32 {
    status.code = 0;
    let guard = STORED_VTABLE.lock().unwrap();
    if let Some(vtable) = guard.as_ref() {
        let mut cb_status = new_cb_status();
        (vtable.get_value)(handle, &mut cb_status)
    } else {
        -1
    }
}

// --- Blocking cross-thread test ---

static THREAD_RESULT: AtomicI32 = AtomicI32::new(0);
static THREAD_DONE: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_use_vtable_from_thread(handle: u64, status: &mut RustCallStatus) {
    status.code = 0;
    THREAD_DONE.store(false, Ordering::SeqCst);
    let get_value = STORED_VTABLE
        .lock()
        .unwrap()
        .as_ref()
        .map(|vt| vt.get_value);
    if let Some(get_value) = get_value {
        std::thread::spawn(move || {
            let mut cb_status = new_cb_status();
            let result = (get_value)(handle, &mut cb_status);
            THREAD_RESULT.store(result, Ordering::SeqCst);
            THREAD_DONE.store(true, Ordering::SeqCst);
        });
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_is_thread_done(status: &mut RustCallStatus) -> i8 {
    status.code = 0;
    if THREAD_DONE.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_get_thread_result(status: &mut RustCallStatus) -> i32 {
    status.code = 0;
    THREAD_RESULT.load(Ordering::SeqCst)
}

// --- Non-blocking cross-thread test ---

#[repr(C)]
pub struct NotifyVTable {
    pub notify: extern "C" fn(u64),
}

static STORED_NOTIFY_VTABLE: Mutex<Option<NotifyVTable>> = Mutex::new(None);
static NOTIFY_DONE: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_init_notify_vtable(
    vtable: &NotifyVTable,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    *STORED_NOTIFY_VTABLE.lock().unwrap() = Some(NotifyVTable {
        notify: vtable.notify,
    });
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_notify_from_thread(handle: u64, status: &mut RustCallStatus) {
    status.code = 0;
    NOTIFY_DONE.store(false, Ordering::SeqCst);
    let notify = STORED_NOTIFY_VTABLE
        .lock()
        .unwrap()
        .as_ref()
        .map(|vt| vt.notify);
    if let Some(notify) = notify {
        std::thread::spawn(move || {
            (notify)(handle);
            NOTIFY_DONE.store(true, Ordering::SeqCst);
        });
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_is_notify_done(status: &mut RustCallStatus) -> i8 {
    status.code = 0;
    if NOTIFY_DONE.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

// --- VTable with RustBuffer callback arg ---

#[repr(C)]
pub struct BufferProcessorVTable {
    pub process: extern "C" fn(u64, RustBuffer, &mut RustCallStatus) -> u32,
    pub free: extern "C" fn(u64, &mut RustCallStatus),
}

static STORED_BUFFER_VTABLE: Mutex<Option<BufferProcessorVTable>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_init_buffer_vtable(
    vtable: &BufferProcessorVTable,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    *STORED_BUFFER_VTABLE.lock().unwrap() = Some(BufferProcessorVTable {
        process: vtable.process,
        free: vtable.free,
    });
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_use_buffer_vtable(
    handle: u64,
    status: &mut RustCallStatus,
) -> u32 {
    status.code = 0;
    let guard = STORED_BUFFER_VTABLE.lock().unwrap();
    if let Some(vtable) = guard.as_ref() {
        let mut cb_status = new_cb_status();
        (vtable.process)(handle, alloc_rustbuffer(&[1, 2, 3, 4, 5]), &mut cb_status)
    } else {
        0
    }
}

static BUFFER_THREAD_RESULT: AtomicI32 = AtomicI32::new(0);
static BUFFER_THREAD_DONE: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_use_buffer_vtable_from_thread(
    handle: u64,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    BUFFER_THREAD_DONE.store(false, Ordering::SeqCst);
    let process = STORED_BUFFER_VTABLE
        .lock()
        .unwrap()
        .as_ref()
        .map(|vt| vt.process);
    if let Some(process) = process {
        std::thread::spawn(move || {
            let mut cb_status = new_cb_status();
            let result = (process)(handle, alloc_rustbuffer(&[10, 20, 30]), &mut cb_status);
            BUFFER_THREAD_RESULT.store(result as i32, Ordering::SeqCst);
            BUFFER_THREAD_DONE.store(true, Ordering::SeqCst);
        });
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_is_buffer_thread_done(status: &mut RustCallStatus) -> i8 {
    status.code = 0;
    if BUFFER_THREAD_DONE.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_get_buffer_thread_result(status: &mut RustCallStatus) -> i32 {
    status.code = 0;
    BUFFER_THREAD_RESULT.load(Ordering::SeqCst)
}

// --- VTable with callback that returns RustBuffer ---

#[repr(C)]
pub struct BufferReturnerVTable {
    pub get_data: extern "C" fn(u64, &mut RustCallStatus) -> RustBuffer,
    pub free: extern "C" fn(u64, &mut RustCallStatus),
}

static STORED_BUFFER_RETURNER_VTABLE: Mutex<Option<BufferReturnerVTable>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_init_buffer_returner_vtable(
    vtable: &BufferReturnerVTable,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    *STORED_BUFFER_RETURNER_VTABLE.lock().unwrap() = Some(BufferReturnerVTable {
        get_data: vtable.get_data,
        free: vtable.free,
    });
}

/// Calls get_data, reads the returned buffer, sums the bytes, frees the buffer, returns the sum.
#[no_mangle]
pub extern "C" fn uniffi_test_fn_use_buffer_returner(
    handle: u64,
    status: &mut RustCallStatus,
) -> u32 {
    status.code = 0;
    let guard = STORED_BUFFER_RETURNER_VTABLE.lock().unwrap();
    if let Some(vtable) = guard.as_ref() {
        let mut cb_status = new_cb_status();
        let buf = (vtable.get_data)(handle, &mut cb_status);
        let len = buf.len as usize;
        let mut sum: u32 = 0;
        if len > 0 && !buf.data.is_null() {
            for i in 0..len {
                sum += unsafe { *buf.data.add(i) } as u32;
            }
        }
        free_buffer(buf);
        sum
    } else {
        0
    }
}

static RETURNER_THREAD_RESULT: AtomicI32 = AtomicI32::new(0);
static RETURNER_THREAD_DONE: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_use_buffer_returner_from_thread(
    handle: u64,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    RETURNER_THREAD_DONE.store(false, Ordering::SeqCst);
    let get_data = STORED_BUFFER_RETURNER_VTABLE
        .lock()
        .unwrap()
        .as_ref()
        .map(|vt| vt.get_data);
    if let Some(get_data) = get_data {
        std::thread::spawn(move || {
            let mut cb_status = new_cb_status();
            let buf = (get_data)(handle, &mut cb_status);
            let len = buf.len as usize;
            let mut sum: u32 = 0;
            if len > 0 && !buf.data.is_null() {
                for i in 0..len {
                    sum += unsafe { *buf.data.add(i) } as u32;
                }
            }
            free_buffer(buf);
            RETURNER_THREAD_RESULT.store(sum as i32, Ordering::SeqCst);
            RETURNER_THREAD_DONE.store(true, Ordering::SeqCst);
        });
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_is_returner_thread_done(status: &mut RustCallStatus) -> i8 {
    status.code = 0;
    if RETURNER_THREAD_DONE.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_get_returner_thread_result(status: &mut RustCallStatus) -> i32 {
    status.code = 0;
    RETURNER_THREAD_RESULT.load(Ordering::SeqCst)
}

// --- ScalarEchoVTable: test all scalar types through VTable arg+ret ---

#[repr(C)]
pub struct ScalarEchoVTable {
    pub echo_u8: extern "C" fn(u64, u8, &mut RustCallStatus) -> u8,
    pub echo_i8: extern "C" fn(u64, i8, &mut RustCallStatus) -> i8,
    pub echo_u16: extern "C" fn(u64, u16, &mut RustCallStatus) -> u16,
    pub echo_i16: extern "C" fn(u64, i16, &mut RustCallStatus) -> i16,
    pub echo_u32: extern "C" fn(u64, u32, &mut RustCallStatus) -> u32,
    pub echo_i32: extern "C" fn(u64, i32, &mut RustCallStatus) -> i32,
    pub echo_u64: extern "C" fn(u64, u64, &mut RustCallStatus) -> u64,
    pub echo_i64: extern "C" fn(u64, i64, &mut RustCallStatus) -> i64,
    pub echo_f32: extern "C" fn(u64, f32, &mut RustCallStatus) -> f32,
    pub echo_f64: extern "C" fn(u64, f64, &mut RustCallStatus) -> f64,
    pub free: extern "C" fn(u64, &mut RustCallStatus),
}

static STORED_SCALAR_ECHO_VTABLE: Mutex<Option<ScalarEchoVTable>> = Mutex::new(None);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_init_scalar_echo_vtable(
    vtable: &ScalarEchoVTable,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    *STORED_SCALAR_ECHO_VTABLE.lock().unwrap() = Some(ScalarEchoVTable {
        echo_u8: vtable.echo_u8,
        echo_i8: vtable.echo_i8,
        echo_u16: vtable.echo_u16,
        echo_i16: vtable.echo_i16,
        echo_u32: vtable.echo_u32,
        echo_i32: vtable.echo_i32,
        echo_u64: vtable.echo_u64,
        echo_i64: vtable.echo_i64,
        echo_f32: vtable.echo_f32,
        echo_f64: vtable.echo_f64,
        free: vtable.free,
    });
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_u8_via_vtable(
    handle: u64,
    value: u8,
    status: &mut RustCallStatus,
) -> u8 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_u8)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_i8_via_vtable(
    handle: u64,
    value: i8,
    status: &mut RustCallStatus,
) -> i8 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_i8)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_u16_via_vtable(
    handle: u64,
    value: u16,
    status: &mut RustCallStatus,
) -> u16 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_u16)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_i16_via_vtable(
    handle: u64,
    value: i16,
    status: &mut RustCallStatus,
) -> i16 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_i16)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_u32_via_vtable(
    handle: u64,
    value: u32,
    status: &mut RustCallStatus,
) -> u32 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_u32)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_i32_via_vtable(
    handle: u64,
    value: i32,
    status: &mut RustCallStatus,
) -> i32 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_i32)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_u64_via_vtable(
    handle: u64,
    value: u64,
    status: &mut RustCallStatus,
) -> u64 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_u64)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_i64_via_vtable(
    handle: u64,
    value: i64,
    status: &mut RustCallStatus,
) -> i64 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_i64)(handle, value, &mut s)
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_f32_via_vtable(
    handle: u64,
    value: f32,
    status: &mut RustCallStatus,
) -> f32 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_f32)(handle, value, &mut s)
    } else {
        0.0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_f64_via_vtable(
    handle: u64,
    value: f64,
    status: &mut RustCallStatus,
) -> f64 {
    status.code = 0;
    let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
    if let Some(vt) = guard.as_ref() {
        let mut s = new_cb_status();
        (vt.echo_f64)(handle, value, &mut s)
    } else {
        0.0
    }
}

// --- Cross-thread callback with RustBuffer ---

#[no_mangle]
pub extern "C" fn uniffi_test_fn_call_callback_with_buffer_from_thread(
    cb: BufferCallback,
    handle: u64,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(10));
        cb(handle, alloc_rustbuffer(&[0xCA, 0xFE, 0xBA, 0xBE]));
    });
}

static SCALAR_ECHO_THREAD_RESULT: AtomicI32 = AtomicI32::new(0);
static SCALAR_ECHO_THREAD_DONE: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn uniffi_test_fn_echo_all_scalars_via_vtable_from_thread(
    handle: u64,
    status: &mut RustCallStatus,
) {
    status.code = 0;
    SCALAR_ECHO_THREAD_DONE.store(false, Ordering::SeqCst);
    let fns = {
        let guard = STORED_SCALAR_ECHO_VTABLE.lock().unwrap();
        guard.as_ref().map(|vt| {
            (
                vt.echo_u8,
                vt.echo_i8,
                vt.echo_u16,
                vt.echo_i16,
                vt.echo_u32,
                vt.echo_i32,
                vt.echo_u64,
                vt.echo_i64,
                vt.echo_f32,
                vt.echo_f64,
            )
        })
    };
    if let Some((
        echo_u8,
        echo_i8,
        echo_u16,
        echo_i16,
        echo_u32,
        echo_i32,
        echo_u64,
        echo_i64,
        echo_f32,
        echo_f64,
    )) = fns
    {
        std::thread::spawn(move || {
            let mut pass_count: i32 = 0;

            let mut s = new_cb_status();
            if (echo_u8)(handle, 200, &mut s) == 200 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_i8)(handle, -100, &mut s) == -100 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_u16)(handle, 50000, &mut s) == 50000 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_i16)(handle, -30000, &mut s) == -30000 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_u32)(handle, 3_000_000_000, &mut s) == 3_000_000_000 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_i32)(handle, -1_000_000, &mut s) == -1_000_000 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_u64)(handle, 0xDEAD_BEEF_CAFE_BABEu64, &mut s) == 0xDEAD_BEEF_CAFE_BABEu64 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            if (echo_i64)(handle, -0x1234_5678_9ABC_DEF0i64, &mut s) == -0x1234_5678_9ABC_DEF0i64 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            let f32_result = (echo_f32)(handle, 2.5f32, &mut s);
            if (f32_result - 2.5f32).abs() < 0.001 {
                pass_count += 1;
            }

            let mut s = new_cb_status();
            let f64_result = (echo_f64)(handle, 1.23456789012345f64, &mut s);
            if (f64_result - 1.23456789012345f64).abs() < 1e-10 {
                pass_count += 1;
            }

            SCALAR_ECHO_THREAD_RESULT.store(pass_count, Ordering::SeqCst);
            SCALAR_ECHO_THREAD_DONE.store(true, Ordering::SeqCst);
        });
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_is_scalar_echo_thread_done(status: &mut RustCallStatus) -> i8 {
    status.code = 0;
    if SCALAR_ECHO_THREAD_DONE.load(Ordering::SeqCst) {
        1
    } else {
        0
    }
}

#[no_mangle]
pub extern "C" fn uniffi_test_fn_get_scalar_echo_thread_result(status: &mut RustCallStatus) -> i32 {
    status.code = 0;
    SCALAR_ECHO_THREAD_RESULT.load(Ordering::SeqCst)
}
