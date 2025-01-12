pub mod client_side_split;
mod packet;
pub mod server_side_split;
pub mod server_side_vpn_stream;
mod tests;

use std::cell::RefCell;
use std::time::Duration;
use easy_error::Error;
use crate::packet::QueuedPacket;

pub const READ_START_AWAIT_TIMEOUT: Duration = Duration::from_millis(5);

pub trait DataStream: Send {
    fn write_all(&mut self, buf: &[u8]) -> Result<(), Error>;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
    fn shutdown(&mut self);
}

pub struct Split {
    pub data_stream: Box<dyn DataStream>,
    pub filler_stream: Box<dyn DataStream>,
}
pub(crate) struct CommonContext {
    data_pending_packets: RefCell<Vec<QueuedPacket>>,
    filler_pending_packets: RefCell<Vec<QueuedPacket>>,
}

impl CommonContext {
    fn new() -> CommonContext {
        let data_pending_packets: Vec<QueuedPacket> = vec![];
        let filler_pending_packets: Vec<QueuedPacket> = vec![];
        let data_pending_packets = RefCell::new(data_pending_packets);
        let filler_pending_packets = RefCell::new(filler_pending_packets);
        CommonContext { data_pending_packets, filler_pending_packets }
    }
    fn data_to_filler_write(&self, queued_packet: QueuedPacket) {
        self.filler_pending_packets.borrow_mut().push(queued_packet);
    }
    fn data_read(&self) -> Option<QueuedPacket> {
        if !self.data_pending_packets.borrow().is_empty() {
            return Some(self.data_pending_packets.borrow_mut().remove(0));
        }
        None
    }
    fn filler_to_data_write(&self, queued_packet: QueuedPacket) {
        //TODO: добавить проверку переполнения (ограничить)
        self.data_pending_packets.borrow_mut().push(queued_packet);
    }
    fn filler_read(&self) -> Option<QueuedPacket> {
        if !self.filler_pending_packets.borrow().is_empty() {
            return Some(self.filler_pending_packets.borrow_mut().remove(0));
        }
        None
    }
}