use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, Weak,
    },
    task::{RawWaker, RawWakerVTable, Waker},
};

use crate::task::manager::TaskManager;

static TASK_TAG_NUM: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub struct TaskTag(usize);

/// A `Task` represents an asynchronous operation to be executed by an executor.
/// It stores the future that represents the task, as well as a `Spawner` for task management.
pub struct Task {
    pub future: Mutex<Option<Pin<Box<dyn Future<Output = ()> + Send + 'static>>>>, // The task's future.
    pub task_tag: TaskTag,          // Tag associated with the task
    pub manager: Weak<TaskManager>, // Reference to manager
    pub abort: Arc<AtomicBool>,     // Abort the task
}

impl Task {
    /// The waker virtual table used to handle task waking functionality.
    const WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);

    /// Creates a `Waker` for this task, allowing it to be polled by an executor.
    pub fn waker(self: Arc<Self>) -> Waker {
        let opaque_ptr = Arc::into_raw(self) as *const ();
        let vtable = &Self::WAKER_VTABLE;
        unsafe { Waker::from_raw(RawWaker::new(opaque_ptr, vtable)) }
    }

    pub fn generate_tag() -> TaskTag {
        TaskTag(TASK_TAG_NUM.fetch_add(1, Ordering::Relaxed))
    }

    pub fn has_aborted(&self) -> bool {
        self.abort.load(Ordering::SeqCst)
    }
}

/// Clones a `RawWaker` pointer, incrementing the reference count.
fn clone(ptr: *const ()) -> RawWaker {
    let original: Arc<Task> = unsafe { Arc::from_raw(ptr as _) };
    let arc_clone = Arc::clone(&original);
    std::mem::forget(original);
    // std::mem::forget(cloned);

    RawWaker::new(Arc::into_raw(arc_clone) as *const (), &Task::WAKER_VTABLE)
}

/// Drops a `RawWaker`, decrementing the reference count.
fn drop(ptr: *const ()) {
    let _: Arc<Task> = unsafe { Arc::from_raw(ptr as _) };
}

/// Wakes a task by scheduling it back into the executor.
fn wake(ptr: *const ()) {
    let arc: Arc<Task> = unsafe { Arc::from_raw(ptr as _) };
    let tm = arc.manager.upgrade().unwrap();
    tm.register_or_execute_non_blocking_task(arc);
    // let spawner = arc.spawner.clone();
    // spawner.spawn_task(ExecutorTask::Task(arc));
}

/// Wakes a task by reference without consuming the `Arc`.
fn wake_by_ref(ptr: *const ()) {
    let arc: Arc<Task> = unsafe { Arc::from_raw(ptr as _) };
    let tm = arc.manager.upgrade().unwrap();
    tm.register_or_execute_non_blocking_task(Arc::clone(&arc));
    std::mem::forget(arc);
}
