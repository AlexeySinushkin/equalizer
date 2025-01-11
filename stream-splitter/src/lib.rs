pub mod client_side_split;
mod packet;
pub mod server_side_split;
mod tests;

use std::io;

pub trait DataStream: Send {
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()>;
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize>;
    fn shutdown(&mut self);
}

pub struct Split {
    pub data_stream: Box<dyn DataStream>,
    pub filler_stream: Box<dyn DataStream>,
}
