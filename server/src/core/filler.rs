/*
Следит за количеством переданных данных VPN->client
Если полезных данных недостаточно, дает данные (пока пустые пакеты)
Поддерживается максимальный битрейт в течении 3-10 секунд, после чего
идет медленное затухание
*/
use crate::objects::{HotPotatoInfo, Packet, SentPacket, MAX_STAT_COUNT, ONE_PACKET_MAX_SIZE};
use std::ops::{Sub};
use std::time::{Duration, Instant};
const ANALYZE_PERIOD_MS: u64 = 100;
const MIN_BYTES_TO_FILL:usize = 1024;
const OLD_AGE: Duration = Duration::from_millis(ANALYZE_PERIOD_MS);
enum PacketType {
    Data,
    Filler,
}
struct SentPacketType {
    packet: SentPacket,
    packet_type: PacketType,
}

impl SentPacketType {
    fn new_data(size: usize) -> SentPacketType {
        let packet = SentPacket {
            sent_date: Instant::now(),
            sent_size: size,
        };
        let packet_type = PacketType::Data;
        Self {
            packet,
            packet_type,
        }
    }
    fn new_filler(size: usize) -> SentPacketType {
        let packet = SentPacket {
            sent_date: Instant::now(),
            sent_size: size,
        };
        let packet_type = PacketType::Filler;
        Self {
            packet,
            packet_type,
        }
    }
    //старше = было создан раньше этого времени
    fn is_older(&self, time: &Instant) -> bool {
        return &self.packet.sent_date < time;
    }

    //моложе - был создан позже этого времени
    fn is_younger(&self, time: &Instant) -> bool {
        return &self.packet.sent_date > time;
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

    pub fn set_speed(&mut self, speed: usize) {
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
                    }
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
                    PacketType::Data => {
                        result.data_packets[result.data_count] = Some(pack.packet);
                        result.data_count += 1;
                    }
                    PacketType::Filler => {
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

    /**
    подсчитываем сколько отправили за последние ~100ms
    если количество отправленных данных меньше того, которое могли бы отправить на текущей скорости
    возвращаем количество для заполнения
    */
    pub fn get_available_space(&self) -> usize {
        let right = Instant::now();
        let old_threshold = Instant::now().sub(OLD_AGE);
        let mut left = None;
        let mut data_count : i32 = 0;
        for pack in &self.queue {
            if pack.is_younger(&old_threshold) {
                if left.is_none() {
                    left = Some(pack.packet.sent_date);
                }
                data_count += pack.packet.sent_size as i32;
            }
        }
        if let Some(left) = left {
            let duration = (right - left).as_millis() as i32;
            // S = V*t
            let data_capacity = self.speed as i32 * duration;
            let delta: i32 = data_capacity - data_count;
            if delta > 0 {
                if delta > ONE_PACKET_MAX_SIZE as i32 {
                    return ONE_PACKET_MAX_SIZE;
                }
                return delta as usize;
            }
        }
        0
    }

    /*
        Подсчитываем сколько надо доотправить для поддержания скорости
        S = v*t, S = количество байт
     */
    pub fn get_filler_packet(&self) -> Option<Packet> {
        let bytes_to_fill = self.get_available_space();
        if bytes_to_fill > MIN_BYTES_TO_FILL {
            //не отправляем большие пакеты заполнителя - не забиваем канал
            return Some(Packet::new_packet(bytes_to_fill/2));
        }
        None
    }


}

#[cfg(test)]
mod tests {

    use std::thread::sleep;
    use std::time::Duration;
    use log::{info};
    use crate::core::filler::{Filler, OLD_AGE};
    use crate::tests::test_init::initialize_logger;

    pub const INITIAL_SPEED: usize = 1024 * 1024 / 1000;

    #[test]
    #[inline(never)]
    fn filler_test() {
        initialize_logger();
        let mut filler = Filler::new(INITIAL_SPEED);
        filler.data_was_sent(100);
        sleep(Duration::from_millis(5));
        let fill_packet = filler.get_filler_packet();
        let from = 2 * INITIAL_SPEED;
        let to = 3 * INITIAL_SPEED;
        //assert!(fill_packet.is_some());
        let size = fill_packet.unwrap().size;
        info!("{from} < {size} < {to}");
        assert!(size > from && size < to);
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
