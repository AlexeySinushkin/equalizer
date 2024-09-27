
#[cfg(test)]
pub mod test_init {
    use std::sync::Once;
    use log::LevelFilter;
    use simplelog::{ConfigBuilder, format_description, SimpleLogger};

    static INIT: Once = Once::new();
    pub fn initialize_logger() {
        INIT.call_once(|| {
            let config = ConfigBuilder::new()
                .set_time_format_custom(format_description!("[hour]:[minute]:[second].[subsecond]"))
                .build();
            SimpleLogger::init(LevelFilter::Trace, config).expect("Логгер проинициализирован");
        });
    }
}
