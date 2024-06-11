{{- self.import_infra("uniffiRustCallAsync", "async-rust-call") }}

{%- if ci.has_async_callback_interface_definition() %}
private func uniffiTraitInterfaceCallAsync<T>(
    makeCall: @escaping () async throws -> T,
    handleSuccess: @escaping (T) -> (),
    handleError: @escaping (Int8, RustBuffer) -> ()
) -> UniffiForeignFuture {
    let task = Task {
        do {
            handleSuccess(try await makeCall())
        } catch {
            handleError(CALL_UNEXPECTED_ERROR, {{ Type::String.borrow()|lower_fn }}(String(describing: error)))
        }
    }
    let handle = UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.insert(obj: task)
    return UniffiForeignFuture(handle: handle, free: uniffiForeignFutureFree)

}

private func uniffiTraitInterfaceCallAsyncWithError<T, E>(
    makeCall: @escaping () async throws -> T,
    handleSuccess: @escaping (T) -> (),
    handleError: @escaping (Int8, RustBuffer) -> (),
    lowerError: @escaping (E) -> RustBuffer
) -> UniffiForeignFuture {
    let task = Task {
        do {
            handleSuccess(try await makeCall())
        } catch let error as E {
            handleError(CALL_ERROR, lowerError(error))
        } catch {
            handleError(CALL_UNEXPECTED_ERROR, {{ Type::String.borrow()|lower_fn }}(String(describing: error)))
        }
    }
    let handle = UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.insert(obj: task)
    return UniffiForeignFuture(handle: handle, free: uniffiForeignFutureFree)
}

// Borrow the callback handle map implementation to store foreign future handles
// TODO: consolidate the handle-map code (https://github.com/mozilla/uniffi-rs/pull/1823)
fileprivate var UNIFFI_FOREIGN_FUTURE_HANDLE_MAP = UniffiHandleMap<UniffiForeignFutureTask>()

// Protocol for tasks that handle foreign futures.
//
// Defining a protocol allows all tasks to be stored in the same handle map.  This can't be done
// with the task object itself, since has generic parameters.
protocol UniffiForeignFutureTask {
    func cancel()
}

extension Task: UniffiForeignFutureTask {}

private func uniffiForeignFutureFree(handle: UInt64) {
    do {
        let task = try UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.remove(handle: handle)
        // Set the cancellation flag on the task.  If it's still running, the code can check the
        // cancellation flag or call `Task.checkCancellation()`.  If the task has completed, this is
        // a no-op.
        task.cancel()
    } catch {
        print("uniffiForeignFutureFree: handle missing from handlemap")
    }
}

// For testing
public func uniffiForeignFutureHandleCount{{ ci.namespace()|class_name(ci) }}() -> Int {
    UNIFFI_FOREIGN_FUTURE_HANDLE_MAP.count
}

{%- endif %}
