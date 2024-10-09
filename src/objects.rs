use std::net::TcpStream;

use std::time::Instant;
use crate::r#const::{MAX_STAT_COUNT, ONE_PACKET_MAX_SIZE};

pub struct Packet {
    pub size: usize,
    pub buf: [u8; ONE_PACKET_MAX_SIZE]
}

impl Packet {
    pub fn new_packet(size: usize) -> Self {
        Self{size, buf: [0; ONE_PACKET_MAX_SIZE] }
    }
}

#[derive(Copy, Clone)]
pub struct SentPacket {
    pub sent_date: Instant,
    pub sent_size: usize,
}


pub struct CollectedInfo {
    pub target_speed: usize,
    pub data_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    pub data_count: usize,
    pub filler_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    pub filler_count: usize,
}

impl Default for CollectedInfo {
    fn default() -> CollectedInfo {
        CollectedInfo {
            target_speed: 0,
            data_count: 0,
            filler_count: 0,
            data_packets: [None; MAX_STAT_COUNT],
            filler_packets: [None; MAX_STAT_COUNT],
        }
    }
}

//команды в сторону прокси (управление)
pub enum RuntimeCommand {
    SetFiller(TcpStream),
    SetSpeed(usize)
}

//информация о состоянии прокси
pub enum ProxyState {
    SetupComplete,
    Info(CollectedInfo),
    Broken
}


