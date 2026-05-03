use crate::{config::SpellRandomizerConfig, log::log_event};

pub struct SpellRandomizer {
    config: SpellRandomizerConfig,
}

impl SpellRandomizer {
    pub fn new(config: &SpellRandomizerConfig) -> Self {
        if config.allow {
            log_event("spell randomizer is allowed by config but not implemented yet");
        }

        Self {
            config: config.clone(),
        }
    }

    pub fn tick(&mut self) {
        if self.config.allow {
            // 预留入口：以后法术随机逻辑从这里接入。
        }
    }

    pub fn update_config(&mut self, config: &SpellRandomizerConfig) {
        if !self.config.allow && config.allow {
            log_event("spell randomizer allowed from config, but it is not implemented yet");
        }
        self.config = config.clone();
    }
}
