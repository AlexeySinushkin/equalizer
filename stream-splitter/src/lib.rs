pub mod client_side_split;
mod packet;
pub mod server_side_split;
pub mod server_side_vpn_stream;
mod tests;

use std::time::Duration;
use easy_error::Error;

pub const READ_START_AWAIT_TIMEOUT: Duration = Duration::from_millis(1);
pub const MAX_BODY_SIZE: usize = 10 * 1024;
pub trait DataStream: Send {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
    fn shutdown(&mut self);
}

