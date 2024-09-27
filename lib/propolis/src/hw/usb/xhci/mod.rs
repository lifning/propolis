//! Emulated eXtensible Host Controller Interface (xHCI) device.

mod bits;
mod controller;
mod registers;
mod rings;

pub use controller::PciXhci;
