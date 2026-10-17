//! Stand-ins for the GPU tier's types where it does not exist. Decoding on
//! the GPU needs Vulkan dmabuf import and VA-API, which only Linux has;
//! elsewhere (macOS) the client decodes on the CPU, and these let the
//! decode worker keep one code path. None of them can be made: `Gpu` holds
//! an uninhabited field, so there is never a `Decoder` or a `DisplayFrame`.
//! The methods return `io::Result` only for the shape of `gliff_vk`'s.

use std::convert::Infallible;
use std::io;
use std::sync::Arc;

pub struct Gpu {
    pub name: String,
    never: Infallible,
}

pub enum Decoder {}

pub enum DisplayFrame {}

impl Decoder {
    pub fn new(gpu: &Arc<Gpu>, _dual: bool, _width: u32, _height: u32) -> io::Result<Self> {
        match gpu.never {}
    }

    pub fn zoom(&self) -> u32 {
        match *self {}
    }

    pub fn set_zoom(&mut self, _zoom: u32) -> io::Result<u32> {
        match *self {}
    }

    pub fn redraw(&mut self) -> io::Result<Option<DisplayFrame>> {
        match *self {}
    }

    pub fn decode(&mut self, _main: &[u8], _aux: &[u8]) -> io::Result<Option<DisplayFrame>> {
        match *self {}
    }
}
