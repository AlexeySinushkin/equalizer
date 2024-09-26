/*
Следит за количеством переданных данных VPN->client
Если полезных данных недостаточно, дает данные (пока пустые пакеты)
Поддерживается максимальный битрейт в течении 3-10 секунд, после чего
идет медленное затухание (TODO)
*/
use std::ops::Sub;
use std::time::{Duration, Instant};
use crate::packet::{Packet, SentPacket};
use crate::r#const::{ONE_PACKET_MAX_SIZE};

const OLD_AGE: Duration = Duration::from_millis(100);
const MAX_STAT_COUNT: usize = 100;

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
    fn is_younger(&self, time: &Instant)->bool{
        return &self.packet.sent_date>time;
    }
}

pub struct Filler {
    queue: Vec<SentPacketType>,
    //bytes per ms
    speed: usize,
    //время бездействия 2ms  (ожидание пока не придет следующий пакет полезных данных)
    idle_time: Duration,
}


pub struct CollectedInfo {
    data_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    data_size: usize,
    filler_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    filler_size: usize,
}

impl Default for CollectedInfo {
    fn default() -> CollectedInfo {
        CollectedInfo {
            data_size: 0,
            filler_size: 0,
            data_packets: [None; MAX_STAT_COUNT],
            filler_packets: [None; MAX_STAT_COUNT],
        }
    }
}

impl Filler {
    pub fn new(speed: usize, idle_time: Duration) -> Filler {
        let queue: Vec<SentPacketType> = Vec::new();
        if idle_time.ge(&OLD_AGE) {
            panic!("Время бездействия должно быть строго меньше времени пометки устаревания")
        }
        Self { queue, speed, idle_time }
    }

    pub fn data_was_sent(&mut self, amount: usize) {
        self.queue.push(SentPacketType::new_data(amount));
    }

    pub fn filler_was_sent(&mut self, amount: usize) {
        self.queue.push(SentPacketType::new_filler(amount));
    }

    /*
    очищаем информацию о пакетах, которые старше 100мс
     */
    pub fn clean(&mut self) -> CollectedInfo {
        let now = Instant::now();
        let old_threshold = now.sub(OLD_AGE);
        let mut result = CollectedInfo::default();


        while let Some(pack) = self.queue.first() {
            if pack.is_older(&old_threshold) {
                let pack = self.queue.remove(0);
                match pack.packet_type {
                    PacketType::Data =>{
                        result.data_packets[result.data_size] = Some(pack.packet);
                        result.data_size += 1;
                    },
                    PacketType::Filler =>{
                        result.filler_packets[result.filler_size] = Some(pack.packet);
                        result.filler_size += 1;
                    }
                }
                if result.data_size == MAX_STAT_COUNT || result.filler_size == MAX_STAT_COUNT {
                    break;
                }
            } else {
                break;
            }
        }
        result
    }

    pub fn get_fill_bytes(&mut self) -> Option<Packet> {
        let dead_line = Instant::now().sub(self.idle_time);
        //если есть полезный пакет, который был 25мс назад
        //а idle_time 20мс, то возвращаем заполнитель который бы скомпенсировал 25мс паузу
        if let Some(last_data) = self.queue.last() {
            //последний пакет был давно
            if last_data.is_older(&dead_line) {
                //создаем новый заполняющий
                return self.create_filler(dead_line);
            }
        }
        None
    }

    /*
    Подсчитываем какого должен быть размера пакет и создаем
    U = n/t
    n = U*t
     */
    fn create_filler(&self, from: Instant) -> Option<Packet> {
        //скорость в дальнейшем может меняться - считаем каждый раз
        let gap = Instant::now().duration_since(from);
        let mut n = (self.speed as usize) * (gap.as_millis() as usize);
        if n == 0 {
            return None;
        }
        if n > ONE_PACKET_MAX_SIZE {
            n = ONE_PACKET_MAX_SIZE;
        }
        //else TODO увеличить размер проанализировав отправленные данные за последние 100мс
        Some(Packet::new_packet(n))
    }
}

#[cfg(test)]
mod tests {
    use std::ops::Sub;
    use std::thread::sleep;
    use std::time::{Duration, Instant};
    use log::{LevelFilter, trace};
    use simplelog::{Config, SimpleLogger};
    use crate::filler::{Filler, OLD_AGE};
    use crate::r#const::{INITIAL_SPEED, ONE_PACKET_MAX_SIZE};
    use crate::tests::initialize_logger;

    #[test]
    fn create_filler_packet_test() {
        initialize_logger();
        let filler = Filler::new(INITIAL_SPEED, Duration::from_millis(2));
        //при скорости 1МБ/с за 1мс мы должны передать 1024 байт
        let p = filler.create_filler(Instant::now().sub(Duration::from_millis(1u64))).unwrap();
        trace!("Размер пакета {} ", p.size);
        assert!(p.size > 1000 && p.size < ONE_PACKET_MAX_SIZE)
    }

    #[test]
    fn filler_test() {
        let mut filler = Filler::new(INITIAL_SPEED, Duration::from_millis(20));
        filler.data_was_sent(10);
        sleep(Duration::from_millis(25));
        let fill_packet = filler.get_fill_bytes();
        assert!(fill_packet.is_some());

        filler.filler_was_sent(10);
        let fill_packet = filler.get_fill_bytes();
        assert!(fill_packet.is_none());
    }

    #[test]
    fn clean_test() {
        let mut filler = Filler::new(INITIAL_SPEED, Duration::from_millis(20));
        filler.data_was_sent(10);
        sleep(OLD_AGE);
        sleep(Duration::from_millis(1));
        let info = filler.clean();
        assert_eq!(1, info.data_size);

        filler.data_was_sent(10);
        let info = filler.clean();
        assert_eq!(0, info.data_size);
    }
}