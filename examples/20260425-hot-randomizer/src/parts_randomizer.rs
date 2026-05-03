use crate::{config::PartsRandomizerConfig, log::log_event};

pub struct PartsRandomizer {
    config: PartsRandomizerConfig,
}

impl PartsRandomizer {
    pub fn new(config: &PartsRandomizerConfig) -> Self {
        if config.allow {
            log_event("parts randomizer is allowed by config but not implemented yet");
        }

        Self {
            config: config.clone(),
        }
    }

    pub fn tick(&mut self) {
        if self.config.allow {
            // 预留入口：以后防具随机逻辑从这里接入。
        }
    }

    pub fn update_config(&mut self, config: &PartsRandomizerConfig) {
        if !self.config.allow && config.allow {
            log_event("parts randomizer allowed from config, but it is not implemented yet");
        }
        self.config = config.clone();
    }
}
