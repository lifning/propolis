use slog::{error, info, o, Logger};

pub struct RamFb {
    addr: u64,
    width: usize,
    height: usize,
}

impl RamFb {
    pub fn new(addr: u64, width: usize, height: usize) -> Self {
        Self {
            addr,
            width,
            height,
        }
    }
}

enum Framebuffer {
    Uninitialized,
    Initialized(RamFb)
}

pub struct VncServer {
    fb: Framebuffer,
}

impl VncServer {
    pub fn new(port: u16, log: Logger) -> Self {
        unimplemented!()
    }

    pub fn start(&self) {
        unimplemented!()
    }

    pub fn initialize_fb(&mut self, fb: RamFb) {
        self.fb = Framebuffer::Initialized(fb);
    }

    pub fn shutdown(&self) {
        unimplemented!()
    }
}
