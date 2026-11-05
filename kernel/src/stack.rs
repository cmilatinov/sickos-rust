use core::ops::{Deref, DerefMut};

#[repr(align(16))]
pub struct Stack<const N: usize>(pub [u8; N]);

impl<const N: usize> Stack<N> {
    pub const fn new() -> Stack<N> {
        Stack([0; N])
    }

    pub const fn size() -> usize {
        N
    }

    pub fn bottom(&self) -> *const u8 {
        &self.0 as _
    }

    pub fn top(&self) -> *const u8 {
        (&self.0 as *const u8).wrapping_add(N)
    }
}

impl<const N: usize> Deref for Stack<N> {
    type Target = [u8; N];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<const N: usize> DerefMut for Stack<N> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
