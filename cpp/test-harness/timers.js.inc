R"((function() {
    "use strict";

    var tasks = [];
    var nextTaskID = 0;
    var curTime = 0;

    // Offset to convert Date.now() (system clock) to the steady clock used by
    // the C++ event loop.  Set once by the first runMacroTask call.
    var clockOffset = 0;
    var clockInitialized = false;

    // Approximate the current steady-clock time using Date.now() + offset.
    // This avoids stale curTime values during long JS evaluation periods
    // (e.g. under CPU contention from parallel test processes).
    function steadyNow() {
        if (!clockInitialized) return curTime;
        return Date.now() + clockOffset;
    }

    // Return the deadline of the next task, or -1 if there is no task.
    function peekMacroTask() {
        return tasks.length ? tasks[0].deadline : -1;
    }

    // Run the next task if it's time.
    // `tm` is the current time (steady clock from C++).
    function runMacroTask(tm) {
        if (!clockInitialized) {
            clockOffset = tm - Date.now();
            clockInitialized = true;
        }
        curTime = tm;
        if (tasks.length && tasks[0].deadline <= tm) {
            var task = tasks.shift();
            task.fn.apply(undefined, task.args);
        }
    }

    function setTimeout(fn, ms = 0, ...args) {
        var id = nextTaskID++;
        // Use steadyNow() instead of curTime so that deadlines are always
        // based on the actual current time, even during long JS evaluation
        // periods where curTime hasn't been updated by the event loop.
        var now = steadyNow();
        var task = {id, fn, deadline: now + Math.max(0, ms | 0), args};
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

    return {peek: peekMacroTask, run: runMacroTask};
})();
)"
