use std::net::TcpStream;

use std::time::Instant;
//размер одного tcp пакета (как правило не больше 1024 - 10_000 хватит для 100Мбит)
pub const ONE_PACKET_MAX_SIZE: usize = 10_000;
pub const MAX_STAT_COUNT: usize = 10;
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




/*
Информация о пакетах которые были отправлены только-что
200-100 мс назад
Должны быть быстро куда-нибудь переданы или агрегированы
 */
pub struct HotPotatoInfo {
    //такую скорость надо было поддерживать в момент отправки пакетов
    pub target_speed: usize,
    pub data_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    pub data_count: usize,
    pub filler_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    pub filler_count: usize,
}

impl Default for HotPotatoInfo {
    fn default() -> HotPotatoInfo {
        HotPotatoInfo {
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
    Info(HotPotatoInfo),
    Broken
}
