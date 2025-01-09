use crate::objects::SentPacket;
use std::ops::Sub;
use std::time::{Duration, Instant};

const ANALYZE_PERIOD_MS: usize = 100;

pub struct ThrottlerAnalyzer {
    queue: Vec<SentPacket>,
    //сколько байт мы можем отправить максимум за ANALYZE_PERIOD_MS (100мс)
    period_capacity: usize,
}

impl ThrottlerAnalyzer {
    //окно для анализа

    pub fn new(speed: usize) -> ThrottlerAnalyzer {
        let queue: Vec<SentPacket> = Vec::new();
        let period_capacity = speed * ANALYZE_PERIOD_MS;
        Self {
            queue,
            period_capacity,
        }
    }

    pub fn set_speed(&mut self, speed: usize) {
        self.period_capacity = speed * ANALYZE_PERIOD_MS;
    }

    pub fn data_was_sent(&mut self, amount: usize) {
        self.queue.push(SentPacket {
            sent_date: Instant::now(),
            sent_size: amount,
        });
    }

    /**
     *   Получить количество байт которые можно отправить
     */
    pub fn get_available_space(&mut self) -> usize {
        let now = Instant::now();
        let youngest = now.sub(Duration::from_millis(ANALYZE_PERIOD_MS as u64));
        //чистим старую информацию
        while !self.queue.is_empty() {
            if let Some(first) = self.queue.first() {
                if first.sent_date.lt(&youngest) {
                    self.queue.remove(0);
                } else {
                    break;
                }
            }
        }
        //подсчитываем размер пакетов за ANALYZE_PERIOD_MS
        let mut overall_sent_size = 0;
        for sp in self.queue.iter() {
            overall_sent_size += sp.sent_size;
        }
        if self.period_capacity > overall_sent_size {
            return self.period_capacity - overall_sent_size;
        }
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn analyzer_test() {
        /*
        при разрешенной скорости 1024 байта в 1мс
        добавляем 90 пакетов размером в 1024 байт
        get_available_space() - должен вернуть 10240
        спустя 10мс get_available_space() - должен вернуть 124 или больше
        */
        let mut analyzer = ThrottlerAnalyzer::new(1024);
        //между первым и остальными пакетами пауза 10мс
        for i in 0..90 {
            analyzer.data_was_sent(1024);
            if i == 0 {
                sleep(Duration::from_millis(10u64));
            }
        }
        //все пакеты свежие
        let space = analyzer.get_available_space();
        assert_eq!(10240, space);
        //первый пакет становится старым - прошло 110мс
        sleep(Duration::from_millis(10));
        let space = analyzer.get_available_space();
        assert!(space >= 10_000);
    }
}
