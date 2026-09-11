use bitfield_struct::bitfield;
use enumflags2::{BitFlags, bitflags, make_bitflags};
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::arch::tss;
use crate::arch::tss::{TSS, TaskStateSegment};
use crate::cell::SyncUnsafeCell;
use const_format::assertcp;

static GDT: SyncUnsafeCell<GlobalDescriptorTable<5>> = {
    let [tss1, tss2] = LongModeSegmentDescriptor::tss(0).into_raw().into_parts();
    SyncUnsafeCell::new(GlobalDescriptorTable {
        entries: [
            RawSegmentDescriptor::null(),
            SegmentDescriptor::kernel_code().into_raw(),
            SegmentDescriptor::kernel_data().into_raw(),
            tss1,
            tss2,
        ],
    })
};

const NULL_SEGMENT: u64 = RawSegmentDescriptor::null().into_bits();
assertcp!(
    NULL_SEGMENT == 0x0,
    "null descriptor mis-encoded: got {:X}",
    NULL_SEGMENT
);

const KERNEL_CODE_SEGMENT: u64 = SegmentDescriptor::kernel_code().into_raw().into_bits();
assertcp!(
    KERNEL_CODE_SEGMENT == 0x00AF_9B00_0000_FFFF,
    "kernel code descriptor mis-encoded: got {:X}",
    KERNEL_CODE_SEGMENT
);

const KERNEL_DATA_SEGMENT: u64 = SegmentDescriptor::kernel_data().into_raw().into_bits();
assertcp!(
    KERNEL_DATA_SEGMENT == 0x00CF_9300_0000_FFFF,
    "kernel data descriptor mis-encoded: got {:X}",
    KERNEL_DATA_SEGMENT
);

const BASE: u32 = RawSegmentDescriptor::new().with_base(0xDEAD_BEEF).base();
assertcp!(BASE == 0xDEAD_BEEF, "base mis-encoded: got {:X}", BASE);

const LIMIT: u32 = RawSegmentDescriptor::new()
    .with_limit(Limit::new(0xFFFFF))
    .limit();
assertcp!(LIMIT == 0xFFFFF, "limit mis-encoded: got {:X}", LIMIT);

const TSS_SEGMENT: u128 = LongModeSegmentDescriptor::tss(0).into_raw().into_bits();
assertcp!(
    TSS_SEGMENT == 0x0000_0000_0000_0000_0000_8900_0000_0067,
    "tss descriptor mis-encoded: got {:X}",
    TSS_SEGMENT
);

#[derive(Copy, Clone)]
#[repr(transparent)]
pub(crate) struct Limit(u32);

impl Limit {
    const fn new(limit: u32) -> Self {
        assert!(limit <= 0xFFFFF);
        Self(limit & 0xFFFFF)
    }

    const unsafe fn new_unchecked(limit: u32) -> Self {
        Self(limit)
    }

    const fn max() -> Self {
        Self(0xFFFFF)
    }

    const fn zero() -> Self {
        Self(0)
    }
}

#[bitfield(u64, order = Msb)]
pub(crate) struct RawSegmentDescriptor {
    base_high: u8,
    #[bits(4)]
    flags: u8,
    #[bits(4)]
    limit_high: u8,
    #[bits(8)]
    access: RawAccess,
    #[bits(24)]
    base_low: u32,
    limit_low: u16,
}

impl RawSegmentDescriptor {
    const fn null() -> Self {
        RawSegmentDescriptor::from_bits(0)
    }

    const fn with_base(self, base: u32) -> Self {
        self.with_base_high(((base >> 24) & 0xFF) as u8)
            .with_base_low(base & 0xFFFFFF)
    }

    const fn with_limit(self, limit: Limit) -> Self {
        self.with_limit_high(((limit.0 >> 16) & 0xF) as u8)
            .with_limit_low((limit.0 & 0xFFFF) as u16)
    }

    const fn base(&self) -> u32 {
        ((self.base_high() as u32) << 24) | self.base_low()
    }

    const fn limit(&self) -> u32 {
        ((self.limit_high() as u32) << 16) | (self.limit_low() as u32)
    }
}

#[bitfield(u8, order = Msb)]
pub(crate) struct RawAccess {
    present: bool,
    #[bits(2)]
    privilege_level: u8,
    #[bits(1)]
    descriptor_type: u8,
    #[bits(1)]
    segment_type: u8,
    #[bits(1)]
    direction_conforming: u8,
    readable_writable: bool,
    accessed: bool,
}

impl RawAccess {
    const fn with_ty(self, ty: u8) -> Self {
        Self::from_bits(ty & 0xF)
            .with_present(self.present())
            .with_privilege_level(self.privilege_level())
            .with_descriptor_type(self.descriptor_type())
    }
}

#[bitfield(u128, order = Msb)]
pub(crate) struct RawLongModeSegmentDescriptor {
    reserved: u32,
    #[bits(40)]
    base_high: u64,
    #[bits(4)]
    flags: u8,
    #[bits(4)]
    limit_high: u8,
    #[bits(8)]
    access: RawAccess,
    #[bits(24)]
    base_low: u32,
    limit_low: u16,
}

impl RawLongModeSegmentDescriptor {
    const fn with_base(self, base: u64) -> Self {
        self.with_base_high((base >> 24) & 0xFFFFFFFFFF)
            .with_base_low((base & 0xFFFFFF) as u32)
    }

    const fn with_limit(self, limit: Limit) -> Self {
        self.with_limit_high(((limit.0 >> 16) & 0xF) as u8)
            .with_limit_low((limit.0 & 0xFFFF) as u16)
    }

    const fn base(&self) -> u64 {
        (self.base_high() << 24) | self.base_low() as u64
    }

    const fn limit(&self) -> u32 {
        ((self.limit_high() as u32) << 16) | (self.limit_low() as u32)
    }

    const fn into_parts(self) -> [RawSegmentDescriptor; 2] {
        [
            RawSegmentDescriptor::from_bits((self.0 & 0xFFFFFFFFFFFFFFFF) as u64),
            RawSegmentDescriptor::from_bits(((self.0 >> 64) & 0xFFFFFFFFFFFFFFFF) as u64),
        ]
    }
}

pub(crate) struct LongModeSegmentDescriptor {
    base: u64,
    limit: Limit,
    flags: BitFlags<SegmentDescriptorFlag>,
    access: SystemAccess,
}

impl LongModeSegmentDescriptor {
    const fn tss(base: u64) -> Self {
        Self {
            base,
            limit: Limit::new(size_of::<TaskStateSegment>() as u32 - 1),
            flags: make_bitflags!(SegmentDescriptorFlag::{}),
            access: SystemAccess {
                ty: SystemSegmentType::TssAvailable32_64,
                privilege_level: PrivilegeLevel::Kernel,
            },
        }
    }

    const fn into_raw(self) -> RawLongModeSegmentDescriptor {
        RawLongModeSegmentDescriptor::new()
            .with_base(self.base)
            .with_limit(self.limit)
            .with_flags(self.flags.bits_c())
            .with_access(self.access.into_raw())
    }
}

pub(crate) struct SegmentDescriptor {
    base: u32,
    limit: Limit,
    flags: BitFlags<SegmentDescriptorFlag>,
    access: Access,
}

impl SegmentDescriptor {
    const fn kernel_code() -> Self {
        Self::new(
            0,
            Limit::max(),
            make_bitflags!(SegmentDescriptorFlag::{Granularity | LongMode}),
            Access::Code(CodeAccess {
                privilege_level: PrivilegeLevel::Kernel,
                conforming: Conforming::DescriptorPrivilegeLevelOnly,
                readable: true,
            }),
        )
    }

    const fn kernel_data() -> Self {
        Self::new(
            0,
            Limit::max(),
            make_bitflags!(SegmentDescriptorFlag::{Granularity | Size}),
            Access::Data(DataAccess {
                privilege_level: PrivilegeLevel::Kernel,
                direction: Direction::Up,
                writable: true,
            }),
        )
    }

    const fn new(
        base: u32,
        limit: Limit,
        flags: BitFlags<SegmentDescriptorFlag>,
        access: Access,
    ) -> Self {
        Self {
            base,
            limit,
            flags,
            access,
        }
    }

    const fn into_raw(self) -> RawSegmentDescriptor {
        RawSegmentDescriptor::new()
            .with_base(self.base)
            .with_limit(self.limit)
            .with_flags(self.flags.bits_c())
            .with_access(self.access.into_raw())
    }
}

#[repr(C, align(16))]
pub(crate) struct GlobalDescriptorTable<const N: usize> {
    entries: [RawSegmentDescriptor; N],
}

#[derive(Copy, Clone)]
pub(crate) struct SystemAccess {
    ty: SystemSegmentType,
    privilege_level: PrivilegeLevel,
}

impl SystemAccess {
    const fn into_raw(self) -> RawAccess {
        RawAccess::new()
            .with_descriptor_type(DescriptorType::SystemSegment as u8)
            .with_privilege_level(self.privilege_level as u8)
            .with_ty(self.ty as u8)
            .with_present(true)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct DataAccess {
    privilege_level: PrivilegeLevel,
    direction: Direction,
    writable: bool,
}

impl DataAccess {
    const fn into_raw(self) -> RawAccess {
        RawAccess::new()
            .with_descriptor_type(DescriptorType::DataCodeSegment as u8)
            .with_privilege_level(self.privilege_level as u8)
            .with_segment_type(SegmentType::Data as u8)
            .with_direction_conforming(self.direction as u8)
            .with_readable_writable(self.writable)
            .with_accessed(true)
            .with_present(true)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct CodeAccess {
    privilege_level: PrivilegeLevel,
    conforming: Conforming,
    readable: bool,
}

impl CodeAccess {
    const fn into_raw(self) -> RawAccess {
        RawAccess::new()
            .with_descriptor_type(DescriptorType::DataCodeSegment as u8)
            .with_privilege_level(self.privilege_level as u8)
            .with_segment_type(SegmentType::Code as u8)
            .with_direction_conforming(self.conforming as u8)
            .with_readable_writable(self.readable)
            .with_accessed(true)
            .with_present(true)
    }
}

#[derive(Copy, Clone)]
pub(crate) enum Access {
    Data(DataAccess),
    Code(CodeAccess),
}

impl Access {
    const fn into_raw(self) -> RawAccess {
        match self {
            Access::Data(inner) => inner.into_raw(),
            Access::Code(inner) => inner.into_raw(),
        }
    }
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum SystemSegmentType {
    TssAvailable16 = 0x1,
    Ldt = 0x2,
    TssBusy16 = 0x3,
    TssAvailable32_64 = 0x9,
    TssBusy32_64 = 0xB,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum PrivilegeLevel {
    Kernel = 0,
    Driver1 = 1,
    Driver2 = 2,
    User = 3,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum SegmentType {
    Data = 0,
    Code = 1,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum DescriptorType {
    SystemSegment = 0,
    DataCodeSegment = 1,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum Direction {
    Up = 0,
    Down = 1,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
pub(crate) enum Conforming {
    DescriptorPrivilegeLevelOnly = 0,
    LowerPrivilegeLevel = 1,
}

#[derive(IntoPrimitive, TryFromPrimitive, Copy, Clone)]
#[repr(u8)]
#[bitflags]
pub(crate) enum SegmentDescriptorFlag {
    Reserved = 0b0001,
    LongMode = 0b0010,
    Size = 0b0100,
    Granularity = 0b1000,
}

#[repr(C, packed(2))]
pub(crate) struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

pub fn init() {
    tss::init();
    let [tss1, tss2] = LongModeSegmentDescriptor::tss(TSS.as_ptr() as u64)
        .into_raw()
        .into_parts();
    let gdt = unsafe { GDT.get_mut() };
    gdt.entries[3] = tss1;
    gdt.entries[4] = tss2;
    let ptr = DescriptorTablePointer {
        limit: size_of_val(gdt) as u16 - 1,
        base: GDT.as_ptr() as u64,
    };
    unsafe {
        core::arch::asm!("lgdt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        core::arch::asm!(
            "push {sel}",
            "lea {tmp}, [55f + rip]",
            "push {tmp}",
            "retfq",
            "55:",
            sel = in(reg) 0x08u64,
            tmp = lateout(reg) _,
            options(preserves_flags),
        );
        core::arch::asm!("mov ds, {0:x}", in(reg) 0x10u16, options(nostack, preserves_flags));
        core::arch::asm!("mov es, {0:x}", in(reg) 0x10u16, options(nostack, preserves_flags));
        core::arch::asm!("mov ss, {0:x}", in(reg) 0x10u16, options(nostack, preserves_flags));
        core::arch::asm!("mov fs, {0:x}", in(reg) 0x10u16, options(nostack, preserves_flags));
        core::arch::asm!("mov gs, {0:x}", in(reg) 0x10u16, options(nostack, preserves_flags));
        core::arch::asm!("ltr {0:x}", in(reg) 0x18u16, options(nostack, preserves_flags));
    }
}
