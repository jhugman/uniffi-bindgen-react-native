RustBuffer rustbuffer_alloc(int32_t size) {
    RustCallStatus status = { UNIFFI_CALL_STATUS_OK };

    return {{ ci.ffi_rustbuffer_alloc().name() }}(
        size,
        &status
    );
}

RustBuffer rustbuffer_from_bytes(const ForeignBytes& bytes) {
    RustCallStatus status = { UNIFFI_CALL_STATUS_OK };

    return {{ ci.ffi_rustbuffer_from_bytes().name() }}(
        bytes,
        &status
    );
}

void rustbuffer_free(RustBuffer& buf) {
    RustCallStatus status = { UNIFFI_CALL_STATUS_OK };

    {{ ci.ffi_rustbuffer_free().name() }}(
        buf,
        &status
    );
}
