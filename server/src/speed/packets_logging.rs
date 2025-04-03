use std::collections::VecDeque;
use std::fs::OpenOptions;
use std::io::BufWriter;
use std::io::Write;
use std::ops::Sub;
use std::time::{Duration, Instant};
use crate::speed::{SpeedForPeriod, SpeedLogging, TimeSpanSentDataInfo, PERCENT_100};
use num_format::{Locale, ToFormattedString};

impl SpeedLogging {
    pub fn new() -> Self {
        let packets_file = OpenOptions::new()
            .create(true)  // Create the file if it does not exist
            .append(true)  // Open in append mode
            .open("packets.log").unwrap();
        let speed_file = OpenOptions::new()
            .create(true)  // Create the file if it does not exist
            .append(true)  // Open in append mode
            .open("speed.log").unwrap();
        SpeedLogging {
            packets_file: BufWriter::new(packets_file),
            speed_file: BufWriter::new(speed_file),
            start_time: Instant::now()
        }
    }

    pub fn append_new_data_log(&mut self, data: &TimeSpanSentDataInfo) {
        writeln!(self.packets_file, "----append #{}", data.id).unwrap();
        let duration_from_start = (data.from - self.start_time).as_millis();
        let duration_formatted = duration_from_start.to_formatted_string(&Locale::en);
        writeln!(self.packets_file, "data/filler {:06}/{:06} time {}, ", data.data_size, data.filler_size, duration_formatted).unwrap();
    }

    pub fn clear_old_data_log(&mut self, data: &TimeSpanSentDataInfo) {
        writeln!(self.packets_file, "----remove #{}", data.id).unwrap();
    }

    pub fn get_speed_log(&mut self, max_duration: Duration, sent_data: &VecDeque<TimeSpanSentDataInfo>, calculated_speed: &SpeedForPeriod) {
        //должно быть как минимум 2 элемента в очереди, так как последний элемент недостаточно точный
        if sent_data.len() < 2 {
            return;
        }
        let right = sent_data.back().unwrap().from;
        let right_id = sent_data.back().unwrap().id;

        let mut left = None;
        let mut left_id = 0;
        let from_threshold = right.sub(max_duration);

        let mut data_amount: usize = 0;
        let mut filler_amount: usize = 0;
        let mut packets = String::from("");
        for sent_data in sent_data.iter() {
            if sent_data.from >= from_threshold && sent_data.from < right {
                let data = sent_data.data_size;
                let filler = sent_data.filler_size;
                packets.push_str(format!("{},{},", data, filler).as_str());
                data_amount += data;
                filler_amount += filler;
                if left == None {
                    left = Some(sent_data.from);
                    left_id = sent_data.id;
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
        let speed = amount / mills;
        let data_percent = data_amount * PERCENT_100 / amount;
        writeln!(self.speed_file,
        "---- {speed} ({}) from #{left_id} to #{right_id} amount {amount} mills {mills} percent {data_percent} ({})",
                 calculated_speed.speed, calculated_speed.data_percent).unwrap();
        writeln!(self.speed_file, "{}", packets).unwrap();
    }

}

