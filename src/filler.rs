/*
Следит за количеством переданных данных VPN->client
Если полезных данных недостаточно, дает данные (пока пустые пакеты)
Поддерживается максимальный битрейт в течении 3-10 секунд, после чего
идет медленное затухание (TODO)
*/
use std::ops::Sub;
use std::time::{Duration, Instant};
use crate::objects::{HotPotatoInfo, Packet, SentPacket, MAX_STAT_COUNT, ONE_PACKET_MAX_SIZE};
const ANALYZE_PERIOD_MS: u64 = 100;
const PREDICT_MS: usize = 10;
const OLD_AGE: Duration = Duration::from_millis(ANALYZE_PERIOD_MS);
const ALMOST_OLD_AGE: Duration = Duration::from_millis(ANALYZE_PERIOD_MS - PREDICT_MS as u64);


enum PacketType {
    Data,
    Filler
}
struct SentPacketType
{
    packet: SentPacket,
    packet_type: PacketType,
}

impl SentPacketType {
    fn new_data(size: usize) -> SentPacketType {
        let packet = SentPacket { sent_date: Instant::now(), sent_size: size };
        let packet_type = PacketType::Data;
        Self{packet, packet_type}
    }
    fn new_filler(size: usize) -> SentPacketType {
        let packet = SentPacket { sent_date: Instant::now(), sent_size: size };
        let packet_type = PacketType::Filler;
        Self{packet, packet_type}
    }
    //старше = было создан раньше этого времени
    fn is_older(&self, time: &Instant) -> bool{
        return &self.packet.sent_date < time;
    }

    //моложе - был создан позже этого времени
    fn is_younger(&self, time: &Instant) ->bool{
        return &self.packet.sent_date>time;
    }
}

pub struct Filler {
    queue: Vec<SentPacketType>,
    //bytes per ms
    speed: usize,
}

impl Filler {
    pub fn new(speed: usize) -> Filler {
        let queue: Vec<SentPacketType> = Vec::new();
        Self { queue, speed }
    }

    pub fn set_speed(&mut self, speed: usize){
        self.speed = speed;
    }

    pub fn data_was_sent(&mut self, amount: usize) {
        self.queue.push(SentPacketType::new_data(amount));
    }

    pub fn filler_was_sent(&mut self, amount: usize) {
        self.queue.push(SentPacketType::new_filler(amount));
    }

    pub fn clean_almost_full(&mut self) -> Option<HotPotatoInfo> {
        let now = Instant::now();
        let old_threshold = now.sub(OLD_AGE);
        let mut data_count = 0;
        let mut filler_count = 0;
        for i in 0..self.queue.len() {
            let pack = self.queue.get(i).unwrap();
            if pack.is_older(&old_threshold) {
                match pack.packet_type {
                    PacketType::Data => {
                        data_count += 1;
                    },
                    PacketType::Filler => {
                        filler_count += 1;
                    }
                }
                if data_count >= MAX_STAT_COUNT - 1 || filler_count >= MAX_STAT_COUNT - 1 {
                    return Some(self.clean());
                }
            }
        }
        None
    }

    /*
    очищаем информацию о пакетах, которые старше 100мс
     */
    pub fn clean(&mut self) -> HotPotatoInfo {
        let now = Instant::now();
        let old_threshold = now.sub(OLD_AGE);
        let mut result = HotPotatoInfo::default();


        while let Some(pack) = self.queue.first() {
            if pack.is_older(&old_threshold) {
                let pack = self.queue.remove(0);
                match pack.packet_type {
                    PacketType::Data =>{
                        result.data_packets[result.data_count] = Some(pack.packet);
                        result.data_count += 1;
                    },
                    PacketType::Filler =>{
                        result.filler_packets[result.filler_count] = Some(pack.packet);
                        result.filler_count += 1;
                    }
                }
                if result.data_count == MAX_STAT_COUNT || result.filler_count == MAX_STAT_COUNT {
                    break;
                }
            } else {
                break;
            }
        }
        result
    }

    /*
    Если данных набралось за период 90мс
    Подсчитываем сколько надо доотправить для периода в 100мс
     */
    pub fn get_fill_bytes(&mut self) -> Option<Packet> {
        let almost_old_time = Instant::now().sub(ALMOST_OLD_AGE);
        //подсчитываем сколько отправили за последние 90мс
        if let Some(size_90) = self.queue.iter().rev().filter(|sp| {
            sp.is_younger(&almost_old_time)
        }).map(|sp| { sp.packet.sent_size })
            .reduce(|acc_size: usize, size| {
                acc_size + size
            }) {
            //столько мы должны отправить за 100мс
            let size_100 = self.speed * ANALYZE_PERIOD_MS as usize;
            //если есть необходимость дополнять
            if size_100 > size_90 {
                //отдаем в 2 раза меньше, чтобы не забивать канал
                let fill_size = (size_100 - size_90) / 2;
                if fill_size > ONE_PACKET_MAX_SIZE {
                    return Some(Packet::new_packet(ONE_PACKET_MAX_SIZE))
                }
                if fill_size > 0 {
                    return Some(Packet::new_packet(fill_size))
                }
            }
        }
        None
    }

}

#[cfg(test)]
mod tests {
    
    use std::thread::sleep;
    use std::time::Duration;
    
    use crate::filler::{Filler, OLD_AGE};
    use crate::speed::INITIAL_SPEED;

    #[test]
    fn filler_test() {
        let mut filler = Filler::new(INITIAL_SPEED);
        filler.data_was_sent(10);
        sleep(Duration::from_millis(25));
        let fill_packet = filler.get_fill_bytes();
        assert!(fill_packet.is_some());
    }

    #[test]
    fn clean_test() {
        let mut filler = Filler::new(INITIAL_SPEED);
        filler.data_was_sent(10);
        sleep(OLD_AGE);
        sleep(Duration::from_millis(1));
        let info = filler.clean();
        assert_eq!(1, info.data_count);

        filler.data_was_sent(10);
        let info = filler.clean();
        assert_eq!(0, info.data_count);
    }
}