use alloc::sync::Arc;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};

use kspin::SpinNoIrq;
use linked_list::{Adapter, Links, List};

use crate::BaseScheduler;

/// A task wrapper for the [`FifoScheduler`].
///
/// It add extra states to use in [`linked_list::List`].
pub struct FifoTask<T> {
    inner: T,
    links: Links<Self>,
}

unsafe impl<T> Adapter for FifoTask<T> {
    type EntryType = Self;

    #[inline]
    fn to_links(t: &Self) -> &Links<Self> {
        &t.links
    }
}

impl<T> FifoTask<T> {
    /// Creates a new [`FifoTask`] from the inner task struct.
    pub const fn new(inner: T) -> Self {
        Self {
            inner,
            links: Links::new(),
        }
    }

    /// Returns a reference to the inner task struct.
    pub const fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T> Deref for FifoTask<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

/// A simple FIFO (First-In-First-Out) cooperative scheduler.
///
/// When a task is added to the scheduler, it's placed at the end of the ready
/// queue. When picking the next task to run, the head of the ready queue is
/// taken.
///
/// As it's a cooperative scheduler, it does nothing when the timer tick occurs.
///
/// It internally uses a linked list as the ready queue.
pub struct FifoScheduler<T> {
    ready_queue: SpinNoIrq<List<Arc<FifoTask<T>>>>,
    num_tasks: AtomicUsize,
}

impl<T> FifoScheduler<T> {
    /// Creates a new empty [`FifoScheduler`].
    pub const fn new() -> Self {
        Self {
            ready_queue: SpinNoIrq::new(List::new()),
            num_tasks: AtomicUsize::new(0),
        }
    }
    /// get the name of scheduler
    pub fn scheduler_name() -> &'static str {
        "FIFO"
    }
}

impl<T> BaseScheduler for FifoScheduler<T> {
    type SchedItem = Arc<FifoTask<T>>;

    fn init(&mut self) {}

    fn add_task(&mut self, task: Self::SchedItem) {
        self.num_tasks.fetch_add(1, Ordering::AcqRel);
        self.ready_queue.lock().push_back(task);
    }

    fn remove_task(&mut self, task: &Self::SchedItem) -> Option<Self::SchedItem> {
        let res = unsafe { self.ready_queue.lock().remove(task) };
        if res.is_some() {
            // Only decrement the number of tasks if the task is removed.
            self.num_tasks.fetch_sub(1, Ordering::AcqRel);
        }
        res
    }

    fn pick_next_task(&mut self) -> Option<Self::SchedItem> {
        let res = self.ready_queue.lock().pop_front();
        if res.is_some() {
            // Only decrement the number of tasks if the task is picked.
            self.num_tasks.fetch_sub(1, Ordering::AcqRel);
        }
        res
    }

    fn put_prev_task(&mut self, prev: Self::SchedItem, _preempt: bool) {
        self.num_tasks.fetch_add(1, Ordering::AcqRel);
        self.ready_queue.lock().push_back(prev);
    }

    fn task_tick(&mut self, _current: &Self::SchedItem) -> bool {
        false // no reschedule
    }

    fn set_priority(&mut self, _task: &Self::SchedItem, _prio: isize) -> bool {
        false
    }

    fn num_tasks(&self) -> usize {
        // Don't need to lock the ready queue to get the number of tasks.
        self.num_tasks.load(Ordering::Acquire)
    }
}
