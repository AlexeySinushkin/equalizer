use std::ops::Sub;
use std::time::Instant;
use std::time::Duration;
use crate::objects::{CollectedInfo, SentPacket};
use crate::r#const::INITIAL_SPEED;

#[derive(Debug, Default)]
pub struct ClientInfo {
    pub key: String,
    pub percent_data: usize,
    pub percent_filler: usize,
    //скорость установленная извне
    pub target_speed: usize,
    //скорость посчитанная - байт за промежуток времени
    pub calculated_speed: usize,
}

pub trait StatisticCollector {
    fn append_info(&mut self, key: &String, info: CollectedInfo);
    fn clear_info(&mut self, key: &String);
    fn calculate_and_get(&mut self) -> Option<Vec<ClientInfo>>;
}


//заглушка для тестов и работы в режиме службы
#[derive(Default)]
pub struct NoStatistic;

impl StatisticCollector for NoStatistic {
    fn append_info(&mut self, _key: &String, _info: CollectedInfo) {}
    fn clear_info(&mut self, _key: &String) {}
    fn calculate_and_get(&mut self) -> Option<Vec<ClientInfo>> {
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
        return &mut self.collected_info[last_index];
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
    fn append_info(&mut self, key: &String, stat: CollectedInfo) {
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

    fn calculate_and_get(&mut self) -> Option<Vec<ClientInfo>> {
        if !self.collected_info.is_empty() {
            let mut result: Vec<ClientInfo> = vec![];
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
                    let percent_data = data_bytes * 100 / total_size;
                    let percent_filler = filler_bytes * 100 / total_size;
                    let calculated_speed = total_size / ANALYZE_PERIOD.as_millis() as usize;//TODO
                    result.push(ClientInfo {
                        key: instance.key.clone(),
                        target_speed: instance.target_speed,
                        percent_data,
                        percent_filler,
                        calculated_speed,
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
    use std::ops::Sub;
    use std::time::{Duration, Instant};
    use log::info;
    use crate::objects::{CollectedInfo, SentPacket};
    use crate::r#const::INITIAL_SPEED;
    use crate::statistic::{SimpleStatisticCollector, StatisticCollector};

    #[test]
    fn simple_statistic_collector() {
        let mut stat = SimpleStatisticCollector::default();
        let key = "1".to_string();
        let mut collected_info = CollectedInfo::default();
        let fifty_ms_ago = Instant::now().sub(Duration::from_millis(50));
        collected_info.target_speed = INITIAL_SPEED;
        collected_info.data_count = 1;
        collected_info.filler_count = 1;
        collected_info.data_packets[0] = Some(SentPacket { sent_date: fifty_ms_ago, sent_size: 10_000 });
        collected_info.filler_packets[0] = Some(SentPacket { sent_date: fifty_ms_ago, sent_size: 5_000 });

        stat.append_info(&key, collected_info);
        let client_info = stat.calculate_and_get().unwrap();
        assert_eq!(1, client_info.len());
        info!("{:?}", client_info.get(0));
    }
}
