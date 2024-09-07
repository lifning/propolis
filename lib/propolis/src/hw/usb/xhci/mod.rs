//! Emulated eXtensible Host Controller Interface (xHCI) device.

mod bits;
mod controller;
mod registers;
mod ring_reader;

pub use controller::PciXhci;
