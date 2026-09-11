use crate::cell::SyncUnsafeCell;
use const_format::assertcp;
use crate::stack::Stack;

pub(crate) static TSS: SyncUnsafeCell<TaskStateSegment> = SyncUnsafeCell::new(TaskStateSegment::new());
static DOUBLE_FAULT_STACK: SyncUnsafeCell<Stack<4096>> = SyncUnsafeCell::new(Stack::new());

#[repr(C, packed(4))]
pub(super) struct TaskStateSegment {
    _reserved0: u32,
    rsp: [u64; 3],
    _reserved1: u64,
    ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    iomap_base: u16,
}

assertcp!(
    size_of::<TaskStateSegment>() == 104,
    "wrong TSS size, expected 104 bytes, got {} bytes",
    size_of::<TaskStateSegment>()
);

impl TaskStateSegment {
    pub const fn new() -> Self {
        Self {
            _reserved0: 0,
            rsp: [0; 3],
            _reserved1: 0,
            ist: [0; 7],
            _reserved2: 0,
            _reserved3: 0,
            iomap_base: size_of::<TaskStateSegment>() as u16,
        }
    }
}

pub fn init() {
    unsafe { TSS.get_mut() }.ist[0] = unsafe { DOUBLE_FAULT_STACK.get() }.top() as u64;
}
