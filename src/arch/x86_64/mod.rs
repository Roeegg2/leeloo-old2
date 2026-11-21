use core::arch::asm;

#[macro_use]
pub mod serial;

#[inline(always)]
unsafe fn outb(port: u16, value: u8) {
    unsafe {
        asm!("out dx, al", in("al") value, in("dx") port);
    }
}

#[inline(always)]
unsafe fn inb(port: u16) -> u8 {
    let ret: i8;
    unsafe {
        asm!("in al, dx", out("al") ret, in("dx") port);
        ret as _
    }
}
