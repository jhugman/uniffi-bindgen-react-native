{%- let name = callback.name()|ffi_callback_name %}
{%- let ns = name|lower|fmt("uniffi_jsi::{}") %}
// ffi_callback_name = {{ callback.name() }}
namespace {{ ns }} {
    using namespace facebook;

    static {{ name }}
    makeCallbackFunction(jsi::Runtime &rt,
                     std::shared_ptr<react::CallInvoker> callInvoker,
                     const jsi::Function &jsCallback) {
        std::cout << "{{ name }}::makeCallbackFunction called" << std::endl;
        {{ name}} callback;
        try {
            throw jsi::JSError(rt, "VTable callback unimplemented {{ name }}");
        } catch (const std::logic_error &e) {
            throw jsi::JSError(rt, e.what());
        }
    }
} // namespace {{ ns }}
