use std::ops::Sub;
use std::time::Instant;
use std::time::Duration;
use crate::objects::{HotPotatoInfo, SentPacket};
use crate::speed::INITIAL_SPEED;

#[derive(Debug, Default)]
pub struct Summary {
    pub key: String,
    pub percent_data: usize,
    pub percent_filler: usize,
    //скорость установленная извне
    pub target_speed: usize,
    //скорость посчитанная - байт за промежуток времени
    pub calculated_speed: usize,
}

pub trait StatisticCollector {
    fn append_info(&mut self, key: &String, info: HotPotatoInfo);
    fn clear_info(&mut self, key: &String);
    fn calculate_and_get(&mut self) -> Option<Vec<Summary>>;
}


//заглушка для тестов и работы в режиме службы
#[derive(Default)]
pub struct NoStatistic;

impl StatisticCollector for NoStatistic {
    fn append_info(&mut self, _key: &String, _info: HotPotatoInfo) {}
    fn clear_info(&mut self, _key: &String) {}
    fn calculate_and_get(&mut self) -> Option<Vec<Summary>> {
        None
    }
}

const ANALYZE_PERIOD: Duration = Duration::from_millis(300);

/*
Ужимает информацию для отображения в консоле
 */
#[derive(Default)]
pub struct SimpleStatisticCollector {
    collected_info: Vec<CurrentRollingInfo>,
}

impl SimpleStatisticCollector {
    fn get_or_create(&mut self, key: &String) -> &mut CurrentRollingInfo {
        for i in 0..self.collected_info.len() {
            if self.collected_info[i].key.eq(key) {
                return &mut self.collected_info[i];
            }
        }
        let new = CurrentRollingInfo::new(key.clone());
        self.collected_info.push(new);
        let last_index = self.collected_info.len() - 1;
        &mut self.collected_info[last_index]
    }
}


struct CurrentRollingInfo {
    key: String,
    target_speed: usize,
    data: Vec<SentPacket>,
    filler: Vec<SentPacket>,
}

impl CurrentRollingInfo {
    fn new(key: String) -> CurrentRollingInfo {
        let data: Vec<SentPacket> = vec![];
        let filler: Vec<SentPacket> = vec![];
        CurrentRollingInfo {
            key,
            target_speed: INITIAL_SPEED,
            data,
            filler,
        }
    }
}

impl StatisticCollector for SimpleStatisticCollector {
    fn append_info(&mut self, key: &String, stat: HotPotatoInfo) {
        let instance = self.get_or_create(key);
        for i in 0..stat.data_count {
            instance.data.push(stat.data_packets[i].unwrap());
        }
        for i in 0..stat.filler_count {
            instance.filler.push(stat.filler_packets[i].unwrap());
        }
        let old_packets = Instant::now().sub(ANALYZE_PERIOD);
        while let Some(first) = instance.data.first() {
            if first.sent_date < old_packets {
                instance.data.remove(0);
            } else {
                break;
            }
        }
        while let Some(first) = instance.filler.first() {
            if first.sent_date < old_packets {
                instance.filler.remove(0);
            } else {
                break;
            }
        }
    }

    fn clear_info(&mut self, key: &String) {
        for i in 0..self.collected_info.len() {
            if self.collected_info[i].key.eq(key) {
                self.collected_info.remove(i);
                break;
            }
        }
    }

    fn calculate_and_get(&mut self) -> Option<Vec<Summary>> {
        if !self.collected_info.is_empty() {
            let mut result: Vec<Summary> = vec![];
            for instance in self.collected_info.iter() {
                let data_bytes: usize = instance.data.iter()
                    .map(|sp| { sp.sent_size })
                    .reduce(|acc_size: usize, size| {
                        acc_size + size
                    }).unwrap_or_else(|| { 0 });
                let filler_bytes: usize = instance.filler.iter()
                    .map(|sp| { sp.sent_size })
                    .reduce(|acc_size: usize, size| {
                        acc_size + size
                    }).unwrap_or_else(|| { 0 });
                let total_size = data_bytes + filler_bytes;
                if total_size > 0 {
                    let percent_data =  data_bytes * 100 / total_size;
                    let percent_filler = filler_bytes * 100 / total_size;
                    let calculated_speed = total_size / ANALYZE_PERIOD.as_millis() as usize;//TODO
                    result.push(Summary {
                        key: instance.key.clone(),
                        target_speed: instance.target_speed,
                        percent_data,
                        percent_filler,
                        calculated_speed,
                    })
                }else{
                    //даже если посчитать не удалось, отправляем, чтобы не выглядело как ошибка
                    result.push(Summary {
                        key: instance.key.clone(),
                        target_speed: instance.target_speed,
                        percent_data: 0,
                        percent_filler: 0,
                        calculated_speed: 0,
                    })
                }
            }
            return Some(result);
        }
        None
    }
}


#[cfg(test)]
mod tests {
    use std::ops::{Add, Sub};
    use std::time::{Instant};
    use log::info;
    use crate::objects::{HotPotatoInfo, SentPacket};
    use crate::speed::INITIAL_SPEED;
    use crate::statistic::{SimpleStatisticCollector, StatisticCollector, ANALYZE_PERIOD};
    use crate::tests::test_init::initialize_logger;

    /*
    чистится внутренняя очередь, даже если никто данные не потребляет
     */
    #[test]
    fn simple_statistic_collector_memory_leak() {
        initialize_logger();
        let mut stat = SimpleStatisticCollector::default();
        let key = "1".to_string();
        let mut old_time = Instant::now().sub(ANALYZE_PERIOD).sub(ANALYZE_PERIOD);

        //добавляем 10 старых пакетов и 10 которые должны идти в расчет
        let increment = ANALYZE_PERIOD/10;
        for _i in 0..20 {
            let mut collected_info = HotPotatoInfo::default();
            collected_info.target_speed = INITIAL_SPEED;
            collected_info.data_count = 1;
            collected_info.filler_count = 1;
            collected_info.data_packets[0] = Some(SentPacket { sent_date: old_time, sent_size: 10_000 });
            collected_info.filler_packets[0] = Some(SentPacket { sent_date: old_time, sent_size: 5_000 });
            stat.append_info(&key, collected_info);
            old_time = old_time.add(increment);
        }

        let rolling_info = stat.get_or_create(&key);
        assert!(rolling_info.data.len()<=10);
        assert!(rolling_info.filler.len()<=10);

        let client_info = stat.calculate_and_get().unwrap();
        assert_eq!(1, client_info.len());
        let summary = client_info.get(0).unwrap();
        assert_eq!(66, summary.percent_data);
        assert_eq!(33, summary.percent_filler);
        info!("{:?}", summary);
    }

    /*
    Достаточно одного пакета для подсчета
     */
    #[test]
    fn one_packet_statistic() {
        initialize_logger();
        let mut stat = SimpleStatisticCollector::default();
        let key = "1".to_string();
        let mut old_time = Instant::now().sub(ANALYZE_PERIOD/2);

        let mut collected_info = HotPotatoInfo::default();
        collected_info.target_speed = INITIAL_SPEED;
        collected_info.data_count = 1;
        collected_info.filler_count = 0;
        collected_info.data_packets[0] = Some(SentPacket { sent_date: old_time, sent_size: 10_000 });
        stat.append_info(&key, collected_info);

        let client_info = stat.calculate_and_get().unwrap();

        let summary = client_info.get(0).unwrap();
        assert_eq!(100, summary.percent_data);
        info!("{:?}", summary);
    }

    #[test]
    fn simple_stat_duplicate() {
        let mut stat = SimpleStatisticCollector::default();
        let key = "1".to_string();
        stat.get_or_create(&key);
        let key2 = "1".to_string();
        stat.get_or_create(&key2);
        assert_eq!(1, stat.collected_info.len());
    }
}
