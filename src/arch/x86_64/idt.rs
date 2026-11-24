use bitfield_struct::bitfield;

#[allow(dead_code)]
#[bitfield(u128)]
struct GateDescriptor {
    #[bits(16)]
    offset_0: u16,
    #[bits(16)]
    selector: u16,
    #[bits(16)]
    handle: u16,
    #[bits(16)]
    offset_1: u16,
    #[bits(32)]
    offset_2: u32,
    #[bits(32)]
    _reserved_1: u32
}

#[repr(transparent)]
struct Idt([GateDescriptor; 256]);

static IDT: spin::Lazy<Idt> = spin::Lazy::new(|| {
    Idt {

        ..Default::default(),
    }
});

impl GateDescriptor {
    fn new() {

    }
}


