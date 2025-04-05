/*
Получаем текущую статистику отправки данных за LONG_TERM для целей понижения скорости
SHORT_TERM - для целей повышения скорости - TODO
держим пропорцию 80/20 (полезных данных по отношению к заполнителю)
Если за LONG_TERM пропорция снизилась на 70/30 - снижаем скорость
Если не снизилась (не ниже 75/25) и SHORT_TERM перешел в другую сторону 90/10 - повышаем скорость
 */
use crate::objects::HotPotatoInfo;
use crate::speed::modify_collected_info::{append_new_data, clear_old_data};
use crate::speed::speed_calculation::get_speed;
use crate::speed::{Info, SpeedCorrector, SpeedCorrectorCommand, SpeedForPeriod, LONG_TERM, INCREASE_SPEED_PERIOD, SHUTDOWN_SPEED, DECREASE_SPEED_PERIOD, PERCENT_100, ENABLE_SPEED, SpeedSetupParam};
use std::collections::HashMap;
use std::ops::Add;
use std::time::{Instant};
use log::{debug, info, trace};

const TARGET_PERCENT: usize = 80;
//для быстрого отключения филлера при слабом канале
//const LOW_SPEED_PROPORTION: usize = 90;
//свободный ход в %. Если отклонились от целевого значения на эту величину - ничего не предпринимаем.
const FREE_PLAY: usize = 2;
//Если процент полезных данных ниже этого значения - уменьшаем скорость (скорость избыточна)
const DOWN_TRIGGER: usize = TARGET_PERCENT - FREE_PLAY;
//Если процент полезных данных выше этого значения - увеличиваем скорость (скорость недостаточна для компенсации всплеска)
const UP_TRIGGER: usize = TARGET_PERCENT + FREE_PLAY;
const UP_ACCELERATION: usize = 70;
const DOWN_ACCELERATION: usize = 50;


impl SpeedCorrector {
    pub fn new() -> SpeedCorrector {
        Self {
            collected_info: HashMap::new(),
        }
    }

    /**
    Precondition: данные приходят хронологически от старых к новым
    */
    //#[inline(never)]
    pub fn append_and_get(
        &mut self,
        key: &String,
        hp: &HotPotatoInfo,
    ) -> Option<SpeedCorrectorCommand> {
        if !self.collected_info.contains_key(key) {
            self.collected_info.insert(key.clone(), Info::new());
        }
        let info = self.collected_info.get_mut(key).unwrap();
        let before_size = info.sent_data.len();
        let new_id = append_new_data(hp, info);
        clear_old_data(info, LONG_TERM);
        let after_size = info.sent_data.len();
        trace!("#{new_id} before_size: {}, after_size: {}", before_size, after_size);

        let now = if let Some(back) = info.sent_data.back(){
            back.from
        }else{
            Instant::now()
        };
        let last_correction_date = if info.last_speed_command.is_some() {
            let command_time = &info.last_speed_command.as_ref().unwrap().command_time;
            Some(command_time)
        }else{
            None
        };
        let mut command = None;
        if let Some(long_term_speed) = get_speed(LONG_TERM, &info.sent_data) {
            if let Some(log) = info.speed_logging.as_mut() {
                log.get_speed_log(LONG_TERM, &info.sent_data, &long_term_speed);
            }
            trace!("calculated speed {} {}%", long_term_speed.speed, long_term_speed.data_percent);
            if last_correction_date.is_none_or(|time| time.add(INCREASE_SPEED_PERIOD) < now)
                && long_term_speed.data_percent > UP_TRIGGER {
                    debug!("increase due percent {} #{new_id}", long_term_speed.data_percent);
                command = Self::increase_command(&long_term_speed, info);
            } else if last_correction_date.is_none_or(|time| time.add(DECREASE_SPEED_PERIOD) < now)
                && long_term_speed.data_percent < DOWN_TRIGGER {
                debug!("decrease due percent {} #{new_id}", long_term_speed.data_percent);
                command = Self::decrease_command(&long_term_speed);
            }
            //не удалось посчитать скорость, но мы ее уже ранее считали (большие задержки - отпускаем все)
        } else if info.last_speed_command.is_none() {
            info!("Недостаточно данных для анализа - отключаем");
            command = Self::switch_off_command(info);
        }

        if let Some(command) = command {
            match command {
                SpeedCorrectorCommand::SwitchOff => {
                    info.last_speed_command = None;
                }
                SpeedCorrectorCommand::SetSpeed(speed) => {
                    info.last_speed_command = Some(SpeedSetupParam::new(speed, now));
                }
            }
        }
        command
    }

    pub fn clear_info(&mut self, key: &String) {
        let _ = self.collected_info.remove(&key.clone());
    }



    //#[inline(never)]
    fn switch_off_command(info: &Info) -> Option<SpeedCorrectorCommand> {
        if info.last_speed_command.is_some() {
            return Some(SpeedCorrectorCommand::SwitchOff);
        }
        None
    }

    fn decrease_command(current_speed: &SpeedForPeriod) -> Option<SpeedCorrectorCommand> {
        let new_speed  = current_speed.speed as i32 - DOWN_ACCELERATION as i32;
        if new_speed < SHUTDOWN_SPEED as i32 {
            return Some(SpeedCorrectorCommand::SwitchOff)
        }
        Some(SpeedCorrectorCommand::SetSpeed(new_speed as usize))
    }
    fn increase_command(current_speed: &SpeedForPeriod, info: &Info) -> Option<SpeedCorrectorCommand> {
        //предыдущая запрошенная скорость
        if let Some(prev_requested_command) = &info.last_speed_command {
            if current_speed.speed < Self::minus_7p(prev_requested_command.value) {
                debug!("Текущая скорость {} ниже запрошенной {}, (уперлись в пропускную способность)",
                        current_speed.speed, prev_requested_command.value);
                return None;
            }
        }
        //новая увеличенная скорость основанная на данных за последние пол секунды
        if info.last_speed_command.is_none() && current_speed.speed > ENABLE_SPEED {
            Some(SpeedCorrectorCommand::SetSpeed(ENABLE_SPEED))
        }else {
            Some(SpeedCorrectorCommand::SetSpeed(current_speed.speed + UP_ACCELERATION))
        }
    }

    fn minus_7p(value: usize) -> usize{
        value * 93 / PERCENT_100
    }
}

#[cfg(test)]
mod tests {
    use crate::objects::{HotPotatoInfo, SentPacket, MAX_STAT_COUNT};
    use crate::speed::speed_correction::{FREE_PLAY, PERCENT_100, TARGET_PERCENT};
    use crate::speed::{to_native_speed, to_regular_speed, SpeedCorrector, SpeedCorrectorCommand, SHUTDOWN_SPEED};
    use crate::tests::test_init::initialize_logger;
    use log::{debug, info};
    use rand::{Rng};
    use std::ops::{Add, Mul};
    use std::thread::sleep;
    use std::time::Duration;
    use std::time::Instant;

    /**
    Отправляем полезных данных со скоростью 50MBit/s
    Убеждаемся что нам устанавливают скорость 60 (50 на данные, 10 на заполнитель)
     */
    #[test]
    fn increase_speed_limit_test() {
        initialize_logger();
        let key = String::from("test");
        let mut speed_corrector = SpeedCorrector::new();
        let mut rng = rand::rng();
        let bytes_per_ms = to_native_speed(50);

        let mut speed_setup_request = 0;
        let start = Instant::now();
        let mut total_sent_size: usize = 0;
        for _i in 0..70 {
            let from = Instant::now();
            //корректировка каждые пол секунды только - 70*8=560ms
            let duration_ms = rng.random_range(8..15);
            let total_bytes_for_period = bytes_per_ms * duration_ms;
            let duration = Duration::from_millis(duration_ms as u64);
            let parts: usize = rng.random_range(1..MAX_STAT_COUNT);
            let hp = get_mock_hp(total_bytes_for_period, PERCENT_100, parts, from, duration);
            sleep(duration);
            debug!("notifying that sent {total_bytes_for_period} during {duration_ms}ms in {parts} parts");
            total_sent_size += total_bytes_for_period;
            if let Some(speed) = speed_corrector.append_and_get(&key, &hp) {
                if let SpeedCorrectorCommand::SetSpeed(speed) = speed {
                    speed_setup_request = speed;
                    info!("------------ setup to {speed} -------------")
                }
            }
        }
        info!("50 MBit/s in native: {bytes_per_ms}");
        let total_time = Instant::now().duration_since(start).as_millis();
        let bytes_per_ms_sent = total_sent_size / total_time as usize;
        info!("avg speed: {bytes_per_ms_sent} b/ms,  total_time: {total_time} ms, total_bytes: {total_sent_size}");
        let expected_speed_from = to_native_speed(51);
        let expected_speed_to = to_native_speed(60);
        info!("requested_speed: {speed_setup_request}, expected from: {expected_speed_from} to: {expected_speed_to}");
        assert!(speed_setup_request > expected_speed_from);
        assert!(speed_setup_request < expected_speed_to);
    }

    /**
       awaiting switch-off command on low speed
    */
    #[test]
    fn switch_off_test() {
        initialize_logger();
        let key = String::from("test");
        let mut speed_corrector = SpeedCorrector::new();
        let mut rng = rand::rng();
        let high_speed = to_native_speed(5);
        let low_speed = SHUTDOWN_SPEED - 50;
        info!("high_speed: {high_speed}, low_speed: {low_speed}, shutdown_speed: {SHUTDOWN_SPEED}");

        let mut speed_setup_request = 0;
        let start = Instant::now();
        let mut total_sent_size: usize = 0;
        let mut switch_off = false;
        for i in 0..1000 {
            let from = Instant::now();
            //корректировка каждые пол секунды только - 70*8=560ms
            let duration_ms = rng.random_range(8..15);
            let (total_bytes_for_period, proportion) = if i < 70 {
                (high_speed * duration_ms, TARGET_PERCENT + FREE_PLAY*2)
            } else {
                if i == 70 {
                    info!("------ Switching to low speed -------");
                }
                (low_speed * duration_ms, TARGET_PERCENT - FREE_PLAY*2)
            };
            let duration = Duration::from_millis(duration_ms as u64);
            let parts: usize = rng.random_range(1..MAX_STAT_COUNT);
            let hp = get_mock_hp(total_bytes_for_period, proportion, parts, from, duration);
            sleep(duration);
            debug!("notifying that sent {total_bytes_for_period} during {duration_ms}ms in {parts} parts");
            total_sent_size += total_bytes_for_period;
            if let Some(speed) = speed_corrector.append_and_get(&key, &hp) {
                if let SpeedCorrectorCommand::SetSpeed(speed) = speed {
                    speed_setup_request = speed;
                    info!("------------ setup to {speed} -------------");
                } else if let SpeedCorrectorCommand::SwitchOff = speed {
                    info!("------------ switch off -------------");
                    switch_off = true;
                    if i>70 {
                        break;
                    }
                }
            }
        }
        let total_time = Instant::now().duration_since(start).as_millis();
        let bytes_per_ms_sent = total_sent_size / total_time as usize;
        let m_bit_per_s = to_regular_speed(bytes_per_ms_sent);
        let m_bit_setup = to_regular_speed(speed_setup_request);
        info!("Bitrate was {m_bit_per_s} MBit/s, Requested speed {m_bit_setup}");
        info!("avg speed: {bytes_per_ms_sent} b/ms,  total_time: {total_time} ms, total_bytes: {total_sent_size}");
        assert!(speed_setup_request>0);
        assert!(switch_off);
    }

    /*
       let mut rng = rand::rng();

       let mut speed_setup_request = 0;
       for i in 0..210 {
           let from = Instant::now();
           let duration_ms = rng.random_range(5..10);
           let total_bytes_for_period = bytes_per_ms * duration_ms;
           let duration = Duration::from_millis(duration_ms as u64);
           let parts: usize = rng.random_range(1..MAX_STAT_COUNT);
           //имитация схождения
           let percent = match i {
               i if i < 20 => rng.random_range(50..100),
               i if i>=20 && i < 50 => rng.random_range(60..90),
               _ => TARGET_PERCENT + FREE_PLAY + FREE_PLAY,
           };
           let hp = get_hp(total_bytes_for_period, percent, parts, from, duration);
           sleep(duration);
           debug!("notifying that sent {total_bytes_for_period} during {duration_ms}ms in {parts} parts");
           if let Some(speed) = speed_corrector.append_and_get(&key, &hp) {
               speed_setup_request = speed;
               info!("------------ setup to {speed} -------------")
           }
       }
    */

    /**
        Получить сводную информацию о том, что было только-что отправлено
        из parts частей начиная с from в течении duration
        @bytes сколько всего байт было отправлено
        @parts на сколько частей поделить
        @proportion 80 - 80% полезных данных - остальное на заполнитель
    */
    fn get_mock_hp(
        bytes: usize,
        proportion: usize,
        parts: usize,
        from: Instant,
        duration: Duration,
    ) -> HotPotatoInfo {
        let mut hp = HotPotatoInfo::default();
        hp.data_count = parts;
        hp.filler_count = parts;
        let part_bytes = bytes / parts;
        for i in 0..parts {
            let sent_date = from.add(duration.mul(i as u32));
            hp.data_packets[i] = Some(SentPacket {
                sent_date,
                sent_size: part_bytes * proportion / PERCENT_100,
            });
            hp.filler_packets[i] = Some(SentPacket {
                sent_date,
                sent_size: part_bytes * (PERCENT_100 - proportion) / PERCENT_100,
            });
        }
        hp
    }
}
