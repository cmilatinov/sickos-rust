//! Minimal framebuffer access via the Limine-provided linear framebuffer.
//! Placeholder until the real console lands: draws a gradient so a successful
//! boot is visible at a glance.

use limine::framebuffer::Framebuffer;

pub fn splash(fb: &Framebuffer) {
    let addr = fb.addr();
    let pitch = fb.pitch() as usize;
    let width = fb.width() as usize;
    let height = fb.height() as usize;
    let bytes_pp = (fb.bpp() as usize).div_ceil(8);

    if bytes_pp != 4 {
        crate::println!("framebuffer: unsupported bpp {}", fb.bpp());
        return;
    }

    for y in 0..height {
        for x in 0..width {
            let r = (x * 255 / width) as u32;
            let b = (y * 255 / height) as u32;
            let pixel = (r << 16) | (0x28 << 8) | b;
            unsafe {
                let p = addr.add(y * pitch + x * 4).cast::<u32>();
                p.write_volatile(pixel);
            }
        }
    }
}
