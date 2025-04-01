use crate::speed::{SpeedForPeriod, TimeSpanSentDataInfo, PERCENT_100};
use std::ops::Sub;
use std::time::Duration;
use log::debug;

pub fn get_speed(
    max_duration: Duration,
    sent_data: &Vec<TimeSpanSentDataInfo>,
) -> Option<SpeedForPeriod> {
    //должно быть как минимум 2 элемента в очереди, так как последний элемент недостаточно точный
    if sent_data.len() < 2 {
        debug!("A few data for speed calculation");
        return None;
    }
    let right = sent_data.last().unwrap().from;
    let mut left = None;
    let from_threshold = right.sub(max_duration);

    let mut data_amount: usize = 0;
    let mut filler_amount: usize = 0;

    for sent_data in sent_data.iter() {
        if sent_data.from >= from_threshold && sent_data.from < right {
            data_amount += sent_data.data_size;
            filler_amount += sent_data.filler_size;
            if left == None {
                left = Some(sent_data.from);
            }
        } else {
            break;
        }
    }
    let amount = data_amount + filler_amount;
    let mills = if let Some(left) = left {
        (right - left).as_millis() as usize
    } else {
        0
    };
    if mills == 0 || amount == 0 {
        debug!("mills {mills} amount {amount}");
        return None;
    }
    Some(SpeedForPeriod {
        speed: amount / mills,
        data_percent: data_amount * PERCENT_100 / amount,
    })
}
