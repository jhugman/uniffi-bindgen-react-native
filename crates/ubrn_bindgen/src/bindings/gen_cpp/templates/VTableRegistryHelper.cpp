// In other uniffi bindings, it is assumed that the foreign language holds on
// to the vtable, which the Rust just gets a pointer to.
// Here, we need to hold on to them, but also be able to clear them at just the
// right time so we can support hot-reloading.
namespace {{ registry }} {
    template <typename T>
    class VTableHolder {
    public:
        T vtable;
        VTableHolder(T v) : vtable(v) {}
    };

    // Mutex to bind the storage and setting of vtable together.
    // We declare it here, but the lock is taken by callers of the putTable
    // method who are also sending a pointer to Rust.
    static std::mutex vtableMutex;

    // Registry to hold all vtables so they persist even when JS objects are GC'd.
    // The only reason this exists is to prevent a dangling pointer in the
    // Rust machinery: i.e. we don't need to access or write to this registry
    // after startup.
    // Registry to hold all vtables so they persist even when JS objects are GC'd.
    // Maps string identifiers to vtable holders using type erasure
    static std::unordered_map<std::string, std::shared_ptr<void>> vtableRegistry;

    // Add a vtable to the registry with an identifier
    template <typename T>
    static T* putTable(std::string_view identifier, T vtable) {
        auto holder = std::make_shared<VTableHolder<T>>(vtable);
        // Store the raw pointer to the vtable before type erasure
        T* rawPtr = &(holder->vtable);
        // Store the holder using type erasure with the string identifier
        vtableRegistry[std::string(identifier)] = std::shared_ptr<void>(holder);
        return rawPtr;
    }

    // Clear the registry.
    //
    // Conceptually, this is called after teardown of the module (i.e. after
    // teardown of the jsi::Runtime). However, because Rust is dropping callbacks
    // because the Runtime is being torn down, we must keep the registry intact
    // until after the runtime goes away.
    //
    // Therefore, in practice we should call this when the next runtime is
    // being stood up.
    static void clearRegistry() {
        std::lock_guard<std::mutex> lock(vtableMutex);
        vtableRegistry.clear();
    }
} // namespace {{ registry }}
