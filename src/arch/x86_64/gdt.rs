use core::arch::asm;

use spin::Once;

static GDT: Once<Gdt> = Once::new();

/// NOTE It's repr(packed) to avoid rusc from adding padding to `size`
#[repr(C, packed)]
pub struct DescriptorTablePointer {
    pub size: u16,
    pub offset: u64,
}

#[repr(C)]
pub struct Gdt {
    null: SegmentDescriptor,
    kernel_code: SegmentDescriptor,
    kernel_data: SegmentDescriptor,
    user_data: SegmentDescriptor,
    user_code: SegmentDescriptor,
    tss: [SegmentDescriptor; 2],
}

/// # Safety 
///
/// This function should only be called once during boot
pub unsafe fn init_gdt() {
    let gdt = GDT.call_once(|| {
        Gdt {
            null: SegmentDescriptor::new32(0, 0, 0, 0),
            kernel_code: SegmentDescriptor::new32(0, 0xf_ffff, 0x9a, 0xa),
            kernel_data: SegmentDescriptor::new32(0, 0xf_ffff, 0x92, 0xc),
            user_data: SegmentDescriptor::new32(0, 0xf_ffff, 0xf2, 0xc),
            user_code: SegmentDescriptor::new32(0, 0xf_ffff, 0xfa, 0xa),
            tss: SegmentDescriptor::new64(0, 0, 0, 0),
        }
    });

    let gdtr = DescriptorTablePointer {
        size: (core::mem::size_of::<Gdt>() - 1) as u16,
        offset: gdt as *const Gdt as u64,
    };

    unsafe {load_gdt(&gdtr, 0x8, 0x10);}

    kprintln!("GDT initialized!");
}

#[inline]
unsafe fn load_gdt(gdtr: &DescriptorTablePointer, code_ss: u16, data_ss: u16) {
    unsafe {
        asm!(
            // Make sure interrupts are disabled
            "cli",
            // Load in the new GDT
            "lgdt [{gdtr}]",

            // Reloasd the segments
            "mov ds, {data_sel:x}",
            "mov es, {data_sel:x}",
            "mov fs, {data_sel:x}",
            "mov gs, {data_sel:x}",
            "mov ss, {data_sel:x}",

            // Reload CS (push the new CS the address of `2, then far return to load it in)
            "push {code_sel}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",

            gdtr = in(reg) gdtr,
            code_sel = in(reg) code_ss as u64,
            data_sel = in(reg) data_ss,
            // We can use lateout because we're reloading CS last :)
            tmp = lateout(reg) _,
            options(preserves_flags),
        )
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Default)]
struct SegmentDescriptor(u64);

impl SegmentDescriptor {
    #[inline]
    const fn new32(base: u32, limit: u32, access: u8, flags: u8) -> Self {
        let mut descr = Self(0);
        descr.set_base(base);
        descr.set_limit(limit);
        descr.set_access(access);
        descr.set_flags(flags);
        descr
    }

    #[inline]
	const fn new64(base: u64, limit: u32, access_byte: u8, flags: u8) -> [Self; 2] {
		[
			Self::new32((base & 0xffff_ffff) as _, limit, access_byte, flags),
			Self((base >> 32) & 0xffff_ffff),
		]
	}

    #[inline]
    const fn set_limit(&mut self, limit: u32) {
		self.0 &= !0xffff;
		self.0 &= !(0xf << 48);

		self.0 |= limit as u64 & 0xffff;
		self.0 |= ((limit as u64 >> 16) & 0xf) << 48;
    }

    #[inline]
	const fn set_base(&mut self, base: u32) {
		self.0 &= !(0xff_ffff << 16);
		self.0 &= !(0xff << 56);

		self.0 |= (base as u64 & 0xff_ffff) << 16;
		self.0 |= ((base as u64 >> 24) & 0xff) << 56;
	}

    #[inline]
	const fn set_access(&mut self, access: u8) {
		self.0 &= !(0xff << 40);
		self.0 |= (access as u64) << 40;
	}

	#[inline]
	const fn set_flags(&mut self, flags: u8) {
		self.0 &= !(0x0f << 52);
		self.0 |= ((flags as u64) & 0x0f) << 52;
	}
}
