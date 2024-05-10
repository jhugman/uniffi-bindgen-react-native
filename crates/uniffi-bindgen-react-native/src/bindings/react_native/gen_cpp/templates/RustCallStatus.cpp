constexpr int8_t UNIFFI_CALL_STATUS_OK = 0;
constexpr int8_t UNIFFI_CALL_STATUS_ERROR = 1;
constexpr int8_t UNIFFI_CALL_STATUS_PANIC = 2;

struct RustCallStatus {
    int8_t code;
    RustBuffer error_buf;
};

{#
template <typename F>
void check_rust_call(const RustCallStatus &status, F error_cb) {
    switch (status.code) {
    case 0:
        return;

    case 1:
        if constexpr (!std::is_null_pointer_v<F>) {
            error_cb(status.error_buf)->throw_underlying();
        }
        break;

    case 2:
        if (status.error_buf.len > 0) {
            throw std::runtime_error({{ Type::String.borrow()|lift_fn }}(status.error_buf));
        }

        throw std::runtime_error("A Rust panic has occurred");
    }

    throw std::runtime_error("Unexpected Rust call status");
}

template <typename F, typename EF, typename... Args, typename R = std::invoke_result_t<F, Args..., RustCallStatus *>>
R rust_call(F f, EF error_cb, Args... args) {
    initialize();

    RustCallStatus status = { 0 };

    if constexpr (std::is_void_v<R>) {
        f(args..., &status);
        check_rust_call(status, error_cb);
    } else {
        auto ret = f(args..., &status);
        check_rust_call(status, error_cb);

        return ret;
    }
}
#}
