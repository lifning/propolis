//! Emulated eXtensible Host Controller Interface (xHCI) device.

mod bits;
mod controller;
mod registers;

pub use controller::PciXhci;
