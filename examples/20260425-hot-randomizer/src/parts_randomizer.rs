use crate::{config::PartsRandomizerConfig, log::log_event};

pub struct PartsRandomizer {
    config: PartsRandomizerConfig,
}

impl PartsRandomizer {
    pub fn new(config: &PartsRandomizerConfig) -> Self {
        if config.enable {
            log_event("parts randomizer is enabled by config but not implemented yet");
        }

        Self {
            config: config.clone(),
        }
    }

    pub fn tick(&mut self) {
        if self.config.enable {
            // 预留入口：以后防具随机逻辑从这里接入。
        }
    }

    pub fn update_config(&mut self, config: &PartsRandomizerConfig) {
        if !self.config.enable && config.enable {
            log_event("parts randomizer enabled from config, but it is not implemented yet");
        }
        self.config = config.clone();
    }
}
