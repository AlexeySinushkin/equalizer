/*
Каждый 5-10 секунд смотрим если заполнение было 30%
снижаем скорость на 10%
Если в течении 5секунд заполнитель был меньше 10%
повышаем скорость на 10%
 */

use std::collections::HashMap;
use std::time::{Duration, Instant};
use crate::objects::CollectedInfo;
use crate::statistic::ClientInfo;

pub struct ChangeSpeedRequired {
    pub key: String,
    pub speed: usize,
}

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

    pub fn append_and_get(&mut self, client_info: Vec<ClientInfo>) -> Option<Vec<ChangeSpeedRequired>>
    {
        None
    }
}