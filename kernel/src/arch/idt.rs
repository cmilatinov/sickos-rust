use crate::arch::gdt::PrivilegeLevel;
use crate::cell::SyncUnsafeCell;
use crate::println;
use bitfield_struct::bitfield;
use derive_more::Debug;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use seq_macro::seq;
use x86_64::registers::control::{Cr2, Cr3};
use x86_64::structures::idt::{InterruptStackFrame, InterruptStackFrameValue};

static IDT: SyncUnsafeCell<InterruptDescriptorTable> =
    SyncUnsafeCell::new(InterruptDescriptorTable::new());

pub(crate) struct InterruptDescriptorTable {
    entries: [RawGateDescriptor; 256],
}

impl InterruptDescriptorTable {
    const fn new() -> Self {
        Self {
            entries: [RawGateDescriptor::new(); 256],
        }
    }

    fn with_entries(entries: impl IntoIterator<Item = (usize, GateDescriptor)>) -> Self {
        let mut table = Self {
            entries: seq!(V in 0u8..=255 { [ #( GateDescriptor::system(unhandled::<V> as u64).into_raw(), )* ] }),
        };
        for (index, entry) in entries {
            table.entries[index] = entry.into_raw();
        }
        table
    }
}

#[bitfield(u128, order = Msb)]
pub(crate) struct RawGateDescriptor {
    reserved1: u32,
    #[bits(48)]
    offset_high: u64,
    present: bool,
    #[bits(2)]
    privilege_level: u8,
    #[bits(1)]
    zero: u8,
    #[bits(4)]
    gate_type: u8,
    #[bits(5)]
    reserved2: u8,
    #[bits(3)]
    ist: u8,
    segment_selector: u16,
    offset_low: u16,
}

impl RawGateDescriptor {
    const fn with_offset(self, offset: u64) -> Self {
        self.with_offset_high((offset >> 16) & 0xFFFFFFFFFFFF)
            .with_offset_low(offset as u16)
    }

    const fn offset(&self) -> u64 {
        (self.offset_high() << 16) | self.offset_low() as u64
    }
}

#[derive(Clone, Copy)]
pub(crate) struct GateDescriptor {
    offset: u64,
    segment_selector: u16,
    ist: u8,
    privilege_level: PrivilegeLevel,
    gate_type: GateType,
}

impl GateDescriptor {
    const fn into_raw(self) -> RawGateDescriptor {
        RawGateDescriptor::new()
            .with_reserved1(0)
            .with_reserved2(0)
            .with_zero(0)
            .with_offset(self.offset)
            .with_present(true)
            .with_segment_selector(self.segment_selector)
            .with_privilege_level(self.privilege_level as u8)
            .with_gate_type(self.gate_type as u8)
            .with_ist(self.ist)
    }

    const fn system(offset: u64) -> Self {
        GateDescriptor {
            offset,
            segment_selector: size_of::<crate::arch::gdt::RawSegmentDescriptor>() as u16,
            ist: 0,
            privilege_level: PrivilegeLevel::Kernel,
            gate_type: GateType::InterruptGate,
        }
    }
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum GateType {
    InterruptGate = 0xE,
    TrapGate = 0xF,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone, Debug)]
#[repr(u8)]
pub(crate) enum Interrupt {
    DivideError = 0x0,
    DebugException = 0x1,
    NmiInterrupt = 0x2,
    Breakpoint = 0x3,
    Overflow = 0x4,
    BoundRangeExceeded = 0x5,
    InvalidOpcode = 0x6,
    DeviceNotAvailable = 0x7,
    DoubleFault = 0x8,
    CoprocessorSegmentOverrun = 0x9,
    InvalidTss = 0xA,
    SegmentNotPresent = 0xB,
    StackSegmentFault = 0xC,
    GeneralProtection = 0xD,
    PageFault = 0xE,
    IntelReserved = 0xF,
    FloatingPointError = 0x10,
    AlignmentCheck = 0x11,
    MachineCheck = 0x12,
    SimdFloatingPointException = 0x13,
    VirtualizationException = 0x14,
    ControlProtectionException = 0x15,
    VmmCommunicationException = 0x1D,
    HypervisorInjectionException = 0x1C,
    SecurityException = 0x1E,
}

impl Interrupt {
    pub fn mnemonic(&self) -> &'static str {
        match self {
            Interrupt::DivideError => "#DE",
            Interrupt::DebugException => "#DB",
            Interrupt::NmiInterrupt => "#NMI",
            Interrupt::Breakpoint => "#BP",
            Interrupt::Overflow => "#OF",
            Interrupt::BoundRangeExceeded => "#BR",
            Interrupt::InvalidOpcode => "#UD",
            Interrupt::DeviceNotAvailable => "#NM",
            Interrupt::DoubleFault => "#DF",
            Interrupt::CoprocessorSegmentOverrun => "",
            Interrupt::InvalidTss => "#TS",
            Interrupt::SegmentNotPresent => "#NP",
            Interrupt::StackSegmentFault => "#SS",
            Interrupt::GeneralProtection => "#GP",
            Interrupt::PageFault => "#PF",
            Interrupt::IntelReserved => "",
            Interrupt::FloatingPointError => "#MF",
            Interrupt::AlignmentCheck => "#AC",
            Interrupt::MachineCheck => "#MC",
            Interrupt::SimdFloatingPointException => "#XM",
            Interrupt::VirtualizationException => "#VE",
            Interrupt::ControlProtectionException => "#CP",
            Interrupt::VmmCommunicationException => "#VC",
            Interrupt::HypervisorInjectionException => "#HV",
            Interrupt::SecurityException => "#SX",
        }
    }
}

pub fn init() {
    let idt = unsafe { &mut *IDT.as_ptr() };
    *idt = InterruptDescriptorTable::with_entries([
        (3, GateDescriptor::system(breakpoint as u64)),
        (6, GateDescriptor::system(invalid_opcode as u64)),
        (8, GateDescriptor::system(double_fault as u64)),
        (13, GateDescriptor::system(general_protection as u64)),
        (14, GateDescriptor::system(page_fault as u64)),
    ]);
}

extern "x86-interrupt" fn unhandled<const V: u8>(frame: InterruptStackFrameValue) {
    panic!(
        "unhandled interrupt {V} at {:#x}",
        frame.instruction_pointer
    );
}

extern "x86-interrupt" fn breakpoint(frame: InterruptStackFrameValue) {
    unsafe {
        core::arch::asm!("iretq");
    }
}

extern "x86-interrupt" fn invalid_opcode(frame: InterruptStackFrameValue) {
    loop {}
}

extern "x86-interrupt" fn double_fault(frame: InterruptStackFrameValue) {
    loop {}
}

extern "x86-interrupt" fn general_protection(frame: InterruptStackFrameValue) {
    loop {}
}

extern "x86-interrupt" fn page_fault(frame: InterruptStackFrameValue) {
    loop {}
}

#[inline]
fn exception_header(interrupt: Interrupt, code: Option<u64>) {
    println!(
        "!!! EXCEPTION {:0x} {} {:?} !!!",
        interrupt as u8,
        interrupt.mnemonic(),
        interrupt
    );
}

#[inline]
fn dump_registers(
    registers: impl IntoIterator<Item = StackFrameRegister>,
    frame: &InterruptStackFrame,
) {
    for register in registers {
        println!(
            "  {:#6?} {:#018x}",
            register,
            register_value(&register, frame)
        );
    }
}

#[inline]
fn register_value(register: &StackFrameRegister, frame: &InterruptStackFrame) -> u64 {
    match register {
        StackFrameRegister::Rip => frame.instruction_pointer.as_u64(),
        StackFrameRegister::Rsp => frame.stack_pointer.as_u64(),
        StackFrameRegister::Rflags => frame.cpu_flags.bits(),
        StackFrameRegister::Cr2 => Cr2::read_raw(),
        StackFrameRegister::Cr3 => {
            let (frame, flags) = Cr3::read_raw();
            frame.start_address().as_u64() | u64::from(flags)
        }
    }
}

#[inline]
fn halt() {
    println!("  -- halted");
    loop {
        x86_64::instructions::hlt();
    }
}

#[bitfield(u16, order = Msb)]
struct PageFaultErrorCode {
    sgx: bool,
    reserved: u8,
    shadow_stack: bool,
    protection_key: bool,
    instruction_fetch: bool,
    reserved_write: bool,
    user: bool,
    write: bool,
    present: bool,
}

#[derive(Debug)]
enum StackFrameRegister {
    #[debug("rip")]
    Rip,
    #[debug("rsp")]
    Rsp,
    #[debug("rflags")]
    Rflags,
    #[debug("cr2")]
    Cr2,
    #[debug("cr3")]
    Cr3,
}

#[derive(Debug)]
enum StackFrameSegment {
    #[debug("cs")]
    Code,
    #[debug("ss")]
    Stack,
}
