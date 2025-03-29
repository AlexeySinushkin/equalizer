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

    pub fn get_available_space(&self) -> usize {
        if let Some(last) = self.queue.last() {
            let mut space = self.get_space(&last.packet);
            if self.is_speed_up_required() {
                space = space + space/2;
            }
            if space > ONE_PACKET_MAX_SIZE {
                return ONE_PACKET_MAX_SIZE;
            }
            return space;
        }
        ONE_PACKET_MAX_SIZE
    }

    fn is_speed_up_required(&self) -> bool {
        let old_threshold = Instant::now().sub(OLD_AGE);
        let mut data_sent_amount = 0;
        let mut filler_sent_amount = 0;
        for pack in &self.queue {
            if pack.is_younger(&old_threshold) {
                match pack.packet_type {
                    PacketType::Data => {
                        data_sent_amount += pack.packet.sent_size;
                    },
                    PacketType::Filler => {
                        filler_sent_amount += pack.packet.sent_size;
                    }

                }
            }
        }
        //количество полезных данных больше 80% за последние 100мс
        filler_sent_amount == 0 || data_sent_amount / filler_sent_amount > 5
    }

    /*
        Подсчитываем сколько надо доотправить для поддержания скорости
        S = v*t, S = количество байт
     */
    pub fn get_filler_packet(&self) -> Option<Packet> {
        if let Some(last) = self.queue.last() {
            let last = last.packet;
            let bytes_to_fill = self.get_space(&last);
            if bytes_to_fill > MIN_BYTES_TO_FILL {
                //не отправляем большие пакеты заполнителя - не забиваем канал
                return Some(Packet::new_packet(MIN_BYTES_TO_FILL));
            }
        }
        None
    }

    fn get_space(&self, from_packet: &SentPacket) -> usize {
        let duration_to_now = Instant::now().sub(from_packet.sent_date);
        let duration_sent = Duration::from_millis((from_packet.sent_size / self.speed) as u64);
        if duration_to_now > duration_sent {
            let delta_ms = duration_to_now.sub(duration_sent).as_millis() as usize;
            return self.speed * delta_ms;
        }
        0
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
    fn filler_test() {
        initialize_logger();
        let mut filler = Filler::new(INITIAL_SPEED);
        filler.data_was_sent(1);
        sleep(Duration::from_millis(5));
        let fill_packet = filler.get_filler_packet();
        let from = 4 * INITIAL_SPEED;
        let to = 6 * INITIAL_SPEED;
        assert!(fill_packet.is_some());
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
