use std::time::{Instant};
use crate::r#const::ONE_PACKET_MAX_SIZE;

pub struct Packet {
    pub size: usize,
    pub buf: [u8; ONE_PACKET_MAX_SIZE]
}

impl Packet {
    pub fn new() -> Self {
        Self{size: ONE_PACKET_MAX_SIZE, buf: [0; ONE_PACKET_MAX_SIZE] }
    }
    pub fn new_packet(size: usize) -> Self {
        Self{size, buf: [0; ONE_PACKET_MAX_SIZE] }
    }
}

#[derive(Copy, Clone)]
pub struct SentPacket {
    pub sent_date: Instant,
    pub sent_size: usize,
}