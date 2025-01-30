pub mod speed_correction;

//10 Мбит/с = 1МБ/с = 1048 байт/мс
//pub const INITIAL_SPEED: usize = 1*1024*1024/1000;
//pub const INITIAL_SPEED: usize = 1024 * 1024 / 1000;
pub const INITIAL_SPEED: usize = 324 * 1024 / 1000;
pub const M_COND: usize = (1024 * 1024 / 10) / 1000;
pub const TO_MB: usize = 1024 * 1024;
pub const TO_KB: usize = 1024;
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
