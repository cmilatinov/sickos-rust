use core::cell::UnsafeCell;

#[repr(transparent)]
pub struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Send> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
    pub const fn new(v: T) -> Self {
        Self(UnsafeCell::new(v))
    }

    pub unsafe fn get(&self) -> &T {
        unsafe { &*self.0.get() }
    }

    /// The caller must ensure exclusive access before creating a mutable reference.
    pub fn as_ptr(&self) -> *mut T {
        self.0.get()
    }
}
