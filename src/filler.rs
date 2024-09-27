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

const ANALYZE_PERIOD_MS: u64 = 100;
const PREDICT_MS: usize = 10;
const OLD_AGE: Duration = Duration::from_millis(ANALYZE_PERIOD_MS);
const ALMOST_OLD_AGE: Duration = Duration::from_millis(ANALYZE_PERIOD_MS - PREDICT_MS as u64);
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
    fn is_younger(&self, time: &Instant) ->bool{
        return &self.packet.sent_date>time;
    }
}

pub struct Filler {
    queue: Vec<SentPacketType>,
    //bytes per ms
    speed: usize,
}


pub struct CollectedInfo {
    pub data_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    pub data_count: usize,
    pub filler_packets: [Option<SentPacket>; MAX_STAT_COUNT],
    pub filler_count: usize,
}

impl Default for CollectedInfo {
    fn default() -> CollectedInfo {
        CollectedInfo {
            data_count: 0,
            filler_count: 0,
            data_packets: [None; MAX_STAT_COUNT],
            filler_packets: [None; MAX_STAT_COUNT],
        }
    }
}

impl Filler {
    pub fn new(speed: usize) -> Filler {
        let queue: Vec<SentPacketType> = Vec::new();
        Self { queue, speed }
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
    use log::{info};
    use crate::filler::{Filler, OLD_AGE};
    use crate::r#const::{INITIAL_SPEED, ONE_PACKET_MAX_SIZE};
    use crate::tests::test_init::initialize_logger;

    #[test]
    fn create_filler_packet_test() {
        initialize_logger();
        let filler = Filler::new(INITIAL_SPEED);
        //при скорости 1МБ/с за 1мс мы должны передать 1024 байт
        let p = filler.create_filler(Instant::now().sub(Duration::from_millis(2u64))).unwrap();
        info!("Размер пакета {} ", p.size);
        assert!(p.size > 1000 && p.size < ONE_PACKET_MAX_SIZE)
    }

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