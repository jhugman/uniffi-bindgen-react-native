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
                var heapUsed = info.hermes_allocatedBytes || 0;
                var heapTotal = info.hermes_heapSize || 0;
                // Hermes' mallocSizeEstimate covers off-heap memory it knows
                // about (e.g. ArrayBuffer backing stores registered with the
                // GC). It's the closest analog to Node's `external`.
                var external = info.hermes_mallocSizeEstimate || 0;
                return {
                    rss: heapTotal,
                    heapTotal: heapTotal,
                    heapUsed: heapUsed,
                    external: external,
                    arrayBuffers: 0,
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
