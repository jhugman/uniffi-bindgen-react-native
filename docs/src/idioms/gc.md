## Hermes Garbage Collection and Rust Drop

In Rust, the compile-time memory management is fairly sophisticated: ownership and borrowing is a first class concept and a whole subsystem of the compilation process is called the borrow-checker. When an structure's ownership is not passed on, then it is dropped. When dropped, if it implements the `Drop` trait, the `drop` function can be run. In addition, any members of a dropped object will also be dropped.

In this manner, resources can be closed and memory can be reclaimed. Rust uses the [Resource Acquisition Is Initialization idiom][raii], and its opposite: resource reclamation is deinitialization.

[raii]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[marksweep]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Memory_Management#mark-and-sweep_algorithm

In Javascript, there is a garbage collector. More concretely it uses a [mark-and-sweep garbage collector][marksweep].

At the boundary between the Rust code and Javascript code, Uniffi has to worry about marrying the two programming models.

The Javascript programmer is handling a Javascript object which is facade onto a Rust object. In the literature this is known as a native-peer. For the JS programmer, the mental model would be that once the object becomes unreachable, then sometime in the future, the GC will reclaim the memory.

But for the Rust, things do not get dropped, and cleanup operations don't get done.

There are several possible approaches:

1. Let the Javascript programmer explicitly tell Rust when they are done with a native peer. This is least convenient for the programmer: if they forget to do this, then a potential memory leak occurs.
1. Somehow persuade the garbage collector to tell Rust that something has fallen out of usage. This is convenient for the programmer, but the GC is not guaranteed to be run.
1. Do both: get the GC to do easy things automatically, to avoid memory leaks, but also provide explicit API to destroy the native peer.

### Current status

#### Garbage collected objects trigger a drop call into Rust

The simplest route for this would be to use a [`FinalizationRegistry`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/FinalizationRegistry).

Unfortunately, [this is not yet supported by hermes](https://github.com/facebook/hermes/issues/604). ([New issue](https://github.com/facebook/hermes/issues/1440))

Instead, for every Javascript object constructor called, we create a `DestructibleObject` in C++, that is represented in Javascript but has a [C++ destructor](https://isocpp.org/wiki/faq/dtors).

[At the end of this \[C++\] object's lifetime](https://en.cppreference.com/w/cpp/language/destructor), the destructor is called.

The assumptions here are:

1. GC reclaims the memory through destruction of the C++ object
1. the same C++ is used throughout the JS lifetime, i.e. memory compaction doesn't exist, or if it does, then objects are `move`d rather than cloned.

Additionally, we observe that:

1. Garbage collection may happen later than you think, if at all; especially in short running tests or apps.
1. Garbage collection may happen sooner than you think, especially in Release.
1. If your Rust object depends on a drop function being called, then you should call its `uniffiDestroy` method before losing it.

#### Explicit API for destroying the native peer

For every object, there is a `uniffiDestroy` method. This can be called more than once. Once it is called, calling any methods on that object results in an error.

To make calling this more automatic, in some circumstances it may be useful to use the `uniffiUse` method:

```ts
const result = new MyObject().uniffiUse((obj) => {
    obj.callSomeMethod();
    return obj.callAnotherMethod();
});
```

### Future work

If there is any movement on [hermes' `FinalizationRegistry` support](https://github.com/facebook/hermes/issues/1440), we may well consider moving to this method.
