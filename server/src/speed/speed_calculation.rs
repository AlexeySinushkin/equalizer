use std::ops::{Sub};
use std::time::Duration;
use crate::speed::{SpeedForPeriod, TimeSpanSentDataInfo, PERCENT_100};

pub fn get_speed(max_duration: Duration, sent_data: &Vec<TimeSpanSentDataInfo>) -> Option<SpeedForPeriod> {
    //должно быть как минимум 2 элемента в очереди, так как последний элемент недостаточно точный
    if sent_data.len() < 2 {
        return None;
    }
    let most_right = sent_data
        .last()
        .unwrap()
        .from;
    let from_threshold = most_right.sub(max_duration);

    let mut data_amount: usize = 0;
    let mut filler_amount: usize = 0;
    let mut duration = Duration::default();
    for sent_data in sent_data.iter() {
        if sent_data.from >= from_threshold && sent_data.from < most_right {
            data_amount += sent_data.data_size;
            filler_amount += sent_data.filler_size;
            duration += sent_data.time_span;
        } else {
            break;
        }
    }
    let amount = data_amount + filler_amount;
    let mills = duration.as_millis() as usize;
    if mills == 0 || amount == 0 {
        return None;
    }
    Some(SpeedForPeriod {
        speed: amount/mills,
        data_percent: data_amount*PERCENT_100/amount
    })
}