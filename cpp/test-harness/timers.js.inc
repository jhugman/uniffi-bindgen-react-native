R"((function() {
    "use strict";

    var tasks = [];
    var nextTaskID = 0;
    var curTime = 0;

    // Return the deadline of the next task, or -1 if there is no task.
    function peekMacroTask() {
        return tasks.length ? tasks[0].deadline : -1;
    }

    // Run the next task if it's time.
    // `tm` is the current time.
    function runMacroTask(tm) {
        curTime = tm;
        if (tasks.length && tasks[0].deadline <= tm) {
            var task = tasks.shift();
            task.fn.apply(undefined, task.args);
        }
    }

    function setTimeout(fn, ms = 0, ...args) {
        var id = nextTaskID++;
        var task = {id, fn, deadline: curTime + Math.max(0, ms | 0), args};
        // Insert the task in the sorted list.
        var i = 0;
        for (i = 0; i < tasks.length; ++i) {
            if (tasks[i].deadline > task.deadline) {
                break;
            }
        }
        tasks.splice(i, 0, task);
        return id;
    }

    function clearTimeout(id) {
        for (var i = 0; i < tasks.length; i++) {
            if (tasks[i].id === id) {
                tasks.splice(i, 1);
                break;
            }
        }
    }

    function setImmediate(fn, ...args) {
        return setTimeout(fn, 0, ...args);
    }
    function clearImmediate(id) {
        return clearTimeout(id);
    }

    globalThis.setTimeout = setTimeout;
    globalThis.clearTimeout = clearTimeout;
    globalThis.setImmediate = setImmediate;
    globalThis.clearImmediate = clearImmediate;

    if (typeof globalThis.gc === 'undefined' &&
        typeof globalThis.__hermesGc === 'function') {
        globalThis.gc = globalThis.__hermesGc;
    }

    if (typeof globalThis.process === 'undefined' &&
        typeof globalThis.__hermesHeapInfo === 'function') {
        var hostHeapInfo = globalThis.__hermesHeapInfo;
        globalThis.process = {
            memoryUsage: function() {
                var info = hostHeapInfo();
                // `allocatedBytes` is CURRENT occupancy: live objects plus any
                // garbage a collection hasn't reclaimed yet. Differencing it
                // across a loop measures where the GC happened to be, not how
                // much was allocated — use `totalAllocated` for that.
                var heapUsed = info.hermes_allocatedBytes || 0;
                var heapTotal = info.hermes_heapSize || 0;
                // `externalBytes` is off-heap memory the GC has been told
                // about (e.g. ArrayBuffer backing stores), and unlike
                // mallocSizeEstimate it is populated without the expensive
                // heap walk. Closest analog to Node's `external`.
                var external = info.hermes_externalBytes ||
                    info.hermes_mallocSizeEstimate || 0;
                return {
                    rss: heapTotal,
                    heapTotal: heapTotal,
                    heapUsed: heapUsed,
                    external: external,
                    arrayBuffers: 0,
                    // Beyond Node's shape, and the only sound basis for an
                    // allocation-rate measurement: cumulative bytes allocated
                    // since runtime creation, immune to GC timing.
                    totalAllocated: info.hermes_totalAllocatedBytes || 0,
                    numCollections: info.hermes_numCollections || 0,
                };
            },
        };
    }

    if (typeof globalThis.performance === 'undefined') {
        var hostNow = globalThis.__performanceNow;
        if (typeof hostNow === 'function') {
            var perfOrigin = hostNow();
            globalThis.performance = {
                now: function() { return hostNow() - perfOrigin; }
            };
        } else {
            var perfStart = Date.now();
            globalThis.performance = {
                now: function() { return Date.now() - perfStart; }
            };
        }
    }

    return {peek: peekMacroTask, run: runMacroTask};
})();
)"
