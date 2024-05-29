## Hermes Garbage Collection and Rust Drop

In Rust, the compile-time memory management is fairly sophisticated: ownership and borrowing is a first class concept and a whole subsystem of the compilation process is called the borrow-checker. When an structure's ownership is not passed on, then it is dropped. When dropped, if it implements the `Drop` trait, the `drop` function can be run. In addition, any members of a dropped object will also be dropped.

In this manner, resources can be closed and memory can be reclaimed. Rust uses the [Resource Acquisition Is Initialization idiom][raii], and its opposite: resource reclaimation is deinitialization.

[raii]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[marksweep]: https://developer.mozilla.org/en-US/docs/Web/JavaScript/Memory_Management#mark-and-sweep_algorithm

In Javascript, there is a garbage collector. More concretely it uses a [mark-and-sweep garbage collector][marksweep].

At the boundary between the Rust code and Javascript code, Uniffi has to worry about marrying the two programming models.

The Javascript programmer is handling a Javascript object which is facade onto a Rust object. In the literature this is known as a native-peer. For the JS programmer, the mental model would be that once the object becomes unreachable, then sometime in the future, the GC will reclaim the memory.

But for the Rust, things do not get dropped, and cleanup operations don't get done.

There are several possible approaches:

1. Let the Javascript programmer explicitly tell Rust when they are done with a native peer. This is least convenient for the programmer: if they forget to do this, then a potential memory leak occurs.
1. Somehow persuade the garbage collecter to tell Rust that something has fallen out of usage. This is convenient for the programmer, but the GC is not guaranteed to be run.
1. Do both: get the GC to do easy things automatically, to avoid memory leaks, but also provide explicit API to destroy the native peer.

### Current status

#### Explicit API for destroying the native peer

For every object, there is a `uniffiDestroy` method. This can be called more than once. Once it is called, calling any methods on that object results in an error.

To make calling this more automatic, in some circumstances it may be useful to use the `uniffiUse` method:

```ts
const result = new MyObject().uniffiUse((obj) => {
    obj.callSomeMethod();
    return obj.callAnotherMethod();
});
```

### Future

In the future, the intention is that we keep this explicit API, but also tie the Hermes Garbage collection to Rust.

The simplest route for this is to use a [`FinalizationRegistry`](https://developer.mozilla.org/en-US/docs/Web/JavaScript/Reference/Global_Objects/FinalizationRegistry). Unfortunately, [this is not yet supported by hermes](https://github.com/facebook/hermes/issues/604).
