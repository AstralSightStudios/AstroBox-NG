use std::cell::UnsafeCell;
use std::hash::{Hash, Hasher};
use std::mem::MaybeUninit;
use std::sync::OnceLock;

use dispatch2::{DispatchQoS, DispatchQueue, DispatchQueueAttr, DispatchRetained};
use objc2::rc::Retained;
use objc2::Message;

/// Get a serial dispatch queue to use for all CoreBluetooth operations
pub(crate) fn queue() -> &'static DispatchQueue {
    static CELL: OnceLock<DispatchRetained<DispatchQueue>> = OnceLock::new();
    CELL.get_or_init(|| {
        let utility =
            DispatchQueue::global_queue(dispatch2::GlobalQueueIdentifier::QualityOfService(DispatchQoS::Utility));
        DispatchQueue::new_with_target("Bluest", DispatchQueueAttr::SERIAL, Some(&utility))
    })
}

/// Synchronizes access to an Objective-C object by restricting all access to occur
/// within the context of a single global serial dispatch queue.
///
/// This allows !Send / !Sync Objective-C types to be used from multiple Rust threads.
#[derive(Debug)]
pub(crate) struct Dispatched<T>(UnsafeCell<Retained<T>>);

unsafe impl<T> Send for Dispatched<T> {}
unsafe impl<T> Sync for Dispatched<T> {}

impl<T: Message> Clone for Dispatched<T> {
    fn clone(&self) -> Self {
        unsafe { Dispatched::retain(&**self.0.get()) }
    }
}

impl<T: PartialEq<T>> PartialEq<Dispatched<T>> for Dispatched<T> {
    fn eq(&self, other: &Dispatched<T>) -> bool {
        self.dispatch(|val| (*val).eq(unsafe { other.get() }))
    }
}

impl<T: Hash> Hash for Dispatched<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hasher may not be Send, so we hash our internal value using `DefaultHasher`
        // then add that hash value to `state`
        self.dispatch(|val| {
            let mut state = std::hash::DefaultHasher::new();
            val.hash(&mut state);
            state.finish()
        })
        .hash(state)
    }
}

impl<T: Eq> Eq for Dispatched<T> {}

impl<T> Dispatched<T> {
    /// # Safety
    ///
    /// - It must be safe to access `value` from the context of [`queue()`].
    /// - After calling `new`, `value` must only be accessed from within the context of `queue()`.
    pub unsafe fn new(value: Retained<T>) -> Self {
        Self(UnsafeCell::new(value))
    }

    /// # Safety
    ///
    /// - It must be safe to access `value` from the context of [`queue()`].
    /// - After calling `retain`, `value` must only be accessed from within the context of `queue()`.
    pub unsafe fn retain(value: &T) -> Self
    where
        T: Message,
    {
        Self(UnsafeCell::new(value.retain()))
    }

    pub fn dispatch<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R + Send,
        R: Send,
    {
        let mut ret = MaybeUninit::uninit();
        queue().exec_sync(|| {
            ret.write((f)(unsafe { self.get() }));
        });
        unsafe { ret.assume_init() }
    }

    /// # Safety
    ///
    /// This method must only be called from within the context of `queue()`.
    pub unsafe fn get(&self) -> &T {
        &*self.0.get()
    }
}
