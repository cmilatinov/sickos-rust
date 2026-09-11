use core::cell::UnsafeCell;

#[repr(transparent)]
pub struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Send> Sync for SyncUnsafeCell<T> {}

impl<T> SyncUnsafeCell<T> {
    pub const fn new(v: T) -> Self { Self(UnsafeCell::new(v)) }
    
    pub unsafe fn get_mut(&self) -> &mut T {
        unsafe { &mut *self.0.get() }
    }
    
    pub unsafe fn get(&self) -> &T {
        unsafe { &*self.0.get() }
    }
    
    pub fn as_ptr(&self) -> *const T {
        self.0.get() as *const T
    }
}