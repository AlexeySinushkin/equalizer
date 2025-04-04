use std::collections::VecDeque;
use crate::speed::{SpeedForPeriod, TimeSpanSentDataInfo, PERCENT_100};
use std::ops::Sub;
use std::time::Duration;
use log::debug;

pub fn get_speed(
    max_duration: Duration,
    sent_data: &VecDeque<TimeSpanSentDataInfo>,
) -> Option<SpeedForPeriod> {
    //должно быть как минимум 2 элемента в очереди, так как последний элемент недостаточно точный
    if sent_data.len() < 2 {
        debug!("A few data for speed calculation {}", sent_data.len());
        return None;
    }
    let back = sent_data.back().unwrap();
    let mut front = None;
    let right = back.from;
    let from_threshold = right.sub(max_duration);

    let mut data_amount: usize = 0;
    let mut filler_amount: usize = 0;

    for sent_data in sent_data.iter() {
        if sent_data.from >= from_threshold && sent_data.from <= right {
            data_amount += sent_data.data_size;
            filler_amount += sent_data.filler_size;
            if front.is_none() {
                front = Some(sent_data);
            }
        } else {
            break;
        }
    }
    let amount = data_amount + filler_amount;
    let (mills, front_id) = if let Some(front) = front {
        ((right - front.from).as_millis() as usize, front.id)
    } else {
        (0, 0)
    };

    debug!("Mills {mills} amount {amount}. {}/{} #{} #{}", sent_data.len(), back.id-front_id, front_id, back.id);
    if mills == 0 || amount == 0 {
        return None;
    }
    Some(SpeedForPeriod {
        speed: amount / mills,
        data_percent: data_amount * PERCENT_100 / amount,
    })
}
