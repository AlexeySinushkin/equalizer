/*
Каждый 5-10 секунд смотрим если заполнение было 30%
снижаем скорость на 10%
Если в течении 5секунд заполнитель был меньше 10%
повышаем скорость на 10%
 */

use std::collections::HashMap;
use std::ops::Sub;
use std::time::{Duration, Instant};

use crate::statistic::Summary;

const DUMMY_DURATION: Duration = Duration::from_millis(100);
const OLD_DATA: Duration = Duration::from_secs(5);
pub struct ChangeSpeedRequired {
    pub key: String,
    pub speed: usize,
}
#[warn(dead_code)]
struct AvgSpeed {
    data_percent: usize,
    calculated_speed: usize,
    target_speed: usize,
    from: Instant,
    duration: Duration,
}

pub struct SpeedCorrector {
    collected_info: HashMap<String, Vec<AvgSpeed>>,
}

impl SpeedCorrector {
    pub fn new() -> SpeedCorrector {
        Self {
            collected_info: HashMap::new()
        }
    }

    /**
    Precondition: данные приходят хронологически от старых к новым
    */
    pub fn append_and_get(&mut self, client_info: Vec<Summary>) -> Option<Vec<ChangeSpeedRequired>>
    {
        for info in client_info.into_iter() {
            self.append_info(info)
        }
        None
    }

    fn append_info(&mut self, client_info: Summary) {
        let collected_info = self.get_or_create(&client_info.key);

        let now = Instant::now();
        collected_info.push(AvgSpeed {
            data_percent: client_info.percent_data,
            calculated_speed: client_info.calculated_speed,
            target_speed: client_info.target_speed,
            from: now,
            duration: DUMMY_DURATION,
        });
        //update N-1
        if collected_info.len() > 1 {
            let index = collected_info.len() - 2;
            let n_1 = collected_info.get_mut(index).unwrap();
            n_1.duration = now.sub(n_1.from)
        }

        //удаляем данные старше 5 секунд
        let old_threshold = now.sub(OLD_DATA);
        while let Some(first) = collected_info.first() {
            if first.from<old_threshold{
                collected_info.remove(0);
            } else {
              break;
            }
        }
    }

    fn get_or_create(&mut self, key: &String) -> &mut Vec<AvgSpeed> {
        if !self.collected_info.contains_key(key) {
            self.collected_info.insert(key.clone(), vec![]);
        }
        let vec = self.collected_info.get_mut(key).unwrap();
        vec
    }
}