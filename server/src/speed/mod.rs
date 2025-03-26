use std::collections::HashMap;
use std::time::Instant;
use std::time::Duration;

pub mod speed_correction;
mod modify_collected_info;
mod speed_calculation;

//10 Мбит/с = 1МБ/с = 1048 байт/мс
//pub const INITIAL_SPEED: usize = 1*1024*1024/1000;

//скорость ниже которой мы отключаем филлер (не до жиру - быть бы живу)
pub(crate) const SHUTDOWN_SPEED : usize = 300 * 1024 / 1000;//Убрать ссылки, инициализировать объекты по-требованию
pub const M_COND: usize = (1024 * 1024 / 10) / 1000;//TODO move
pub const TO_MB: usize = 1024 * 1024; //TODO move
pub const TO_KB: usize = 1024;
const PERCENT_100: usize = 100;
pub(crate) const LONG_TERM: Duration = Duration::from_secs(5);
//pub(crate) const SHORT_TERM: Duration = Duration::from_secs(2);
//меняем скорость не чаще этого периода
pub(crate) const MODIFY_PERIOD: Duration = Duration::from_millis(500);

//TODO move
/*
Пересчитать байт/мс в Мбит/с
 */
pub fn native_to_regular(speed: usize) -> String {
    let bit_per_s = speed * 1000 * 8;
    if speed > M_COND {
        return format!("{}MBit", bit_per_s / TO_MB);
    }
    return format!("{}KBit", bit_per_s / TO_KB);
}

/**
    10 Мбит/с = 1МБ/с = 1048 байт/мс
 */
#[cfg(test)]
pub fn to_native_speed(m_bit_per_s: usize) -> usize {
    m_bit_per_s * 105
}
#[cfg(test)]
pub fn to_regular_speed(bytes_ms: usize) -> usize {
    bytes_ms / 105
}

#[derive(Debug, Copy, Clone)]
pub enum SpeedCorrectorCommand {
    SwitchOff,
    SetSpeed(usize),
}

impl PartialEq for SpeedCorrectorCommand {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (SpeedCorrectorCommand::SwitchOff, SpeedCorrectorCommand::SwitchOff) => true,
            (SpeedCorrectorCommand::SetSpeed(a), SpeedCorrectorCommand::SetSpeed(b)) => a == b,
            _ => false,
        }
    }
}

pub struct SpeedCorrector {
    collected_info: HashMap<String, Info>,
}

struct SpeedForPeriod {
    speed: usize,
    data_percent: usize, //0-100
}
#[derive(Default)]
struct Info {
    sent_data: Vec<TimeSpanSentDataInfo>,
    speed_setup: Vec<SetupSpeedHistory>,
}

struct SetupSpeedHistory {
    setup_time: Instant,
    command: SpeedCorrectorCommand,
}
#[allow(dead_code)]
struct TimeSpanSentDataInfo {
    from: Instant,
    time_span: Duration,
    target_speed: Option<usize>,
    data_size: usize,
    filler_size: usize,
}

