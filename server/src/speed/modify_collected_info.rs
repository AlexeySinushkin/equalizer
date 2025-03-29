use std::cmp::{max, min};
use std::ops::Sub;
use std::time::Instant;
use log::warn;
use crate::objects::HotPotatoInfo;
use crate::speed::{Info, TimeSpanSentDataInfo, HISTORY_HOLD_PERIOD, LONG_TERM};

pub fn append_new_data(hp: &HotPotatoInfo, info: &mut Info) {
    let now = Instant::now();
    //самое раннее время отправки
    let mut min_instant = now;
    //самое позднее время отправки
    let mut max_instant = now;
    let data_size: usize = match hp.data_count > 0 {
        true => {
            let mut result = 0;
            for i in 0..hp.data_count {
                let sent_packet = hp.data_packets[i].unwrap();
                result += sent_packet.sent_size;
                min_instant = min(min_instant, sent_packet.sent_date);
                max_instant = max(max_instant, sent_packet.sent_date);
            }
            result
        }
        false => 0,
    };
    let filler_size: usize = match hp.filler_count > 0 {
        true => {
            let mut result = 0;
            for i in 0..hp.filler_count {
                let sent_packet = hp.filler_packets[i].unwrap();
                result += sent_packet.sent_size;
                min_instant = min(min_instant, sent_packet.sent_date);
                max_instant = max(max_instant, sent_packet.sent_date);
            }
            result
        }
        false => 0,
    };


    info.sent_data.push(TimeSpanSentDataInfo {
        from: min_instant,
        data_size,
        filler_size,
    });
}

pub fn clear_old_data(right_time: Instant, info: &mut Info) {
    //удаляем данные старше 5 секунд
    let old_threshold = right_time.sub(LONG_TERM);
    while let Some(first) = info.sent_data.first() {
        if first.from < old_threshold {
            info.sent_data.remove(0);
        } else {
            break;
        }
    }
    if !info.speed_history.is_empty() {
        let old_threshold = right_time.sub(HISTORY_HOLD_PERIOD);
        while let Some(first) = info.speed_history.first() {
            if first.setup_time < old_threshold {
                info.speed_history.remove(0);
            } else {
                break;
            }
        }
        if info.speed_history.is_empty(){
            warn!("Очередь установки скорости пуста!")
        }
    }
}