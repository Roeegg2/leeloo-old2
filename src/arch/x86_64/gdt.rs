use core::arch::asm;
use core::mem;
use core::cell::UnsafeCell;

use bitfield_struct::bitfield;
use spin::Once;

const KERNEL_STACK_SIZE: usize = 8192;

/// is this segment entry valid or not
const ACCESS_P: u8        = 1 << 7;
/// privilege levels:
const ACCESS_RING_0: u8         = 0 << 5;
const ACCESS_RING_3: u8         = 3 << 5;
///  code/data if true, TSS/LDT otherwise
const ACCESS_DESCR_TYPE: u8   = 1 << 4;
/// if clear the segment is data segment, otherwise it's a code segment
const ACCESS_E: u8     = 1 << 3;
// /// for data segments: direction bit - 1 if the segment grows down, and 0 for up
// /// for code segments: sets min ring level (i.e max privilege level) in which code in this segment can be executed
// const ACCESS_CONFORMING: u8     = 1 << 2;
/// for code segments: marks whether the segment is also readable
/// for data segments: marks whether the segment is also writeable
const ACCESS_RW: u8             = 1 << 1;
/// the CPU will set this bit if the segment is accessed
const ACCESS_A: u8       = 1 << 0;
/// sets the type of the entry as TSS
const ACCESS_TYPE_TSS_AVAIL: u8 = 0x9;

/// the size `limit` is scaled by. 0 means 1 bytes (i.e no scaling), 1 means 4KiB (i.e page)
const FLAGS_GRANULARITY_4K: u8  = 1 << 3;
const FLAGS_SIZE_32: u8         = 1 << 2;
/// whether the descriptor is a 64-bit code segment or not (used for long/compatibility mode
/// support)
const FLAGS_LONG_MODE: u8       = 1 << 1;

#[repr(C, align(16))]
struct KernelStack(UnsafeCell<[u8; 8192]>);

unsafe impl Sync for KernelStack {}

#[repr(C, packed(4))]
#[derive(Default)]
struct Tss {
    reserved_1: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved_2: u64,
    ist1: u64,
    ist2: u64,
    ist3: u64,
    ist4: u64,
    ist5: u64,
    ist6: u64,
    ist7: u64,
    reserved_3: u64,
    reserved_4: u16,
    iomap_base: u16,
}

/// NOTE it's repr(packed) to avoid rusc from adding padding to `size`
#[repr(C, packed)]
struct DescriptorTablePointer {
    size: u16,
    offset: u64,
}

#[repr(C)]
struct Gdt {
    null: SegmentDescriptor,
    kernel_code: SegmentDescriptor,
    kernel_data: SegmentDescriptor,
    user_data: SegmentDescriptor,
    user_code: SegmentDescriptor,
    tss: [SegmentDescriptor; 2],
}

#[allow(dead_code)]
#[bitfield(u64)]
struct SegmentDescriptor {
    #[bits(16)]
    limit_low: u16,
    #[bits(24)]
    base_low: u32,
    access: u8,
    #[bits(4)]
    limit_high: u8,
    #[bits(4)]
    flags: u8,
    base_high: u8,
}

static GDT: Once<Gdt> = Once::new();

static TSS: Once<Tss> = Once::new();

static KERNEL_STACK: KernelStack = KernelStack(UnsafeCell::new([0; 8192]));

/// # Safety 
///
/// this function should only be called once during boot
pub unsafe fn init_gdt() {
    let tss = TSS.call_once(|| {
        Tss {
            // the stack grows downwards, so point to the end of the array
            rsp0: KERNEL_STACK.0.get() as u64 + KERNEL_STACK_SIZE as u64,
            // set the base past the configured size so disable the iobitmap for now 
            iomap_base: mem::size_of::<Tss>() as u16,
            ..Default::default()
        }
    });

    let (tss_addr, tss_size) = (
        tss as *const Tss as u64,
        mem::size_of::<Tss>() as u32,
    );

    let gdt = GDT.call_once(|| {
        Gdt {
            null: SegmentDescriptor::new32(0, 0, 0, 0),
            kernel_code: SegmentDescriptor::new32(0, 0xf_ffff, ACCESS_P | ACCESS_RING_0 | ACCESS_DESCR_TYPE | ACCESS_E | ACCESS_RW | ACCESS_A, FLAGS_GRANULARITY_4K | FLAGS_LONG_MODE),
            kernel_data: SegmentDescriptor::new32(0, 0xf_ffff, ACCESS_P | ACCESS_RING_0 | ACCESS_DESCR_TYPE | ACCESS_RW | ACCESS_A, FLAGS_GRANULARITY_4K | FLAGS_SIZE_32),
            user_data: SegmentDescriptor::new32(0, 0xf_ffff, ACCESS_P | ACCESS_RING_3 | ACCESS_DESCR_TYPE | ACCESS_RW | ACCESS_A, FLAGS_GRANULARITY_4K | FLAGS_SIZE_32),
            user_code: SegmentDescriptor::new32(0, 0xf_ffff, ACCESS_P | ACCESS_RING_3 | ACCESS_DESCR_TYPE | ACCESS_E | ACCESS_RW | ACCESS_A, FLAGS_GRANULARITY_4K | FLAGS_LONG_MODE),
            tss: SegmentDescriptor::new64(tss_addr, tss_size-1, ACCESS_P | ACCESS_RING_0 | ACCESS_TYPE_TSS_AVAIL, 0x0),
        }
    });

    let gdtr = DescriptorTablePointer {
        size: (mem::size_of::<Gdt>() - 1) as u16,
        offset: gdt as *const Gdt as u64,
    };

    unsafe {
        // the kernel's code and data segments are the 2nd and 3rd respectively
        load_gdt(&gdtr, 0x8, 0x10);
        // the TSS's GDT entry is the 5th descriptor
        load_tss(0x28);
    }


    kprintln!("GDT initialized!");
}


impl SegmentDescriptor {
    #[inline]
    fn new32(base: u32, limit: u32, access: u8, flags: u8) -> Self {
        Self::new()
            .with_limit_low(limit as u16)
            .with_base_low(base & 0xff_ffff)
            .with_access(access)
            .with_limit_high((limit >> 16) as u8)
            .with_flags(flags & 0x0f)
            .with_base_high((base >> 24) as u8)
    }

    #[inline]
    fn new64(base: u64, limit: u32, access: u8, flags: u8) -> [Self; 2] {
        [
            Self::new32((base & 0xffff_ffff) as _, limit, access, flags),
            Self::from(base >> 32),
        ]
    }
}

#[inline]
unsafe fn load_tss(offset: u16) {
    unsafe {
        asm!(
            "ltr {offset:x}",
            offset = in(reg) offset,
            options(nostack, preserves_flags),
        )
    }
}

unsafe fn load_gdt(gdtr: &DescriptorTablePointer, code_ss: u16, data_ss: u16) {
    unsafe {
        asm!(
            // make sure interrupts are disabled
            "cli",
            // load in the new GDT
            "lgdt [{gdtr}]",

            // reloasd the segments
            "mov ds, {data_sel:x}",
            "mov es, {data_sel:x}",
            "mov fs, {data_sel:x}",
            "mov gs, {data_sel:x}",
            "mov ss, {data_sel:x}",

            // reload CS (push the new CS the address of `2, then far return to load it in)
            "push {code_sel}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",

            gdtr = in(reg) gdtr,
            code_sel = in(reg) code_ss as u64,
            data_sel = in(reg) data_ss,
            // we can use lateout because we're reloading CS last :)
            tmp = lateout(reg) _,
            options(preserves_flags),
        )
    }
}
