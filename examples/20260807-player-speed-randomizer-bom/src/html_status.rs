use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::speed_randomizer::SpeedStatus;

const DATA_BLOCK_START: &str =
    r#"<script id="player-speed-randomizer-data" type="application/json">"#;
const DATA_BLOCK_END: &str = "</script>";
const MIN_WRITE_INTERVAL: Duration = Duration::from_secs(1);

pub struct HtmlStatusWriter {
    path: PathBuf,
    last_write: Instant,
}

impl HtmlStatusWriter {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            last_write: Instant::now() - MIN_WRITE_INTERVAL,
        }
    }

    pub fn write_if_due(&mut self, status: SpeedStatus) {
        if self.last_write.elapsed() < MIN_WRITE_INTERVAL {
            return;
        }
        let template = match fs::read_to_string(&self.path) {
            Ok(template) => template,
            Err(_) => {
                let template = default_template();
                if fs::write(&self.path, &template).is_err() {
                    return;
                }
                template
            }
        };
        let Some(updated) = replace_data_block(&template, status) else {
            self.last_write = Instant::now();
            return;
        };
        if updated != template {
            let _ = fs::write(&self.path, updated);
        }
        self.last_write = Instant::now();
    }
}

fn replace_data_block(template: &str, status: SpeedStatus) -> Option<String> {
    let start = template.find(DATA_BLOCK_START)?;
    let content_start = start + DATA_BLOCK_START.len();
    let content_end = content_start + template[content_start..].find(DATA_BLOCK_END)?;
    let mut updated = template.to_string();
    updated.replace_range(
        content_start..content_end,
        &format!("\n{}\n", render_data(status)),
    );
    Some(updated)
}

fn render_data(status: SpeedStatus) -> String {
    format!(
        "{{\n  \"enabled\": {},\n  \"speedEnabled\": {},\n  \"multiplier\": {:.3},\n  \"countdown\": \"{}\"\n}}",
        status.enabled,
        status.speed_enabled,
        status.multiplier,
        format_countdown(status.countdown_ms),
    )
}

fn default_template() -> String {
    r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="1">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Player Speed Randomizer</title>
<style>
:root { color-scheme: dark; font-family: "Segoe UI", "Microsoft YaHei", sans-serif; }
body { margin: 0; background: #111827; color: #e5e7eb; }
main { max-width: 500px; margin: 0 auto; padding: 28px 20px; }
.card { padding: 22px; border: 1px solid #374151; border-radius: 14px; background: #1f2937; }
.value { font-size: 42px; font-weight: 800; }
.countdown { margin-top: 14px; font: 32px Consolas, monospace; color: #93c5fd; }
.state { margin: 12px 0; color: #d1d5db; }
</style>
</head>
<body>
<main><div class="card">
<h1>玩家速度随机</h1>
<div data-sr="state" class="state">状态：关闭</div>
<div data-sr="multiplier" class="value">1.00x</div>
<div>距离下一次随机</div>
<div data-sr="countdown" class="countdown">--:--</div>
</div></main>
<script id="player-speed-randomizer-data" type="application/json">
{"enabled":false,"speedEnabled":false,"multiplier":1.0,"countdown":"--:--"}
</script>
<script>
(() => {
 const data = JSON.parse(document.getElementById("player-speed-randomizer-data").textContent);
 const set = (name, value) => { const el = document.querySelector(`[data-sr="${name}"]`); if (el) el.textContent = value; };
 set("state", `状态：${data.enabled && data.speedEnabled ? "开启" : "关闭"}`);
 set("multiplier", `${Number(data.multiplier).toFixed(2)}x`);
 set("countdown", data.countdown);
})();
</script>
</body>
</html>
"##.to_string()
}

fn format_countdown(countdown_ms: Option<u64>) -> String {
    let Some(countdown_ms) = countdown_ms else {
        return "--:--".to_string();
    };
    let total_seconds = countdown_ms / 1_000;
    format!("{:02}:{:02}", total_seconds / 60, total_seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_uses_minutes_and_seconds() {
        assert_eq!(format_countdown(Some(3_890)), "00:03");
    }

    #[test]
    fn replacement_preserves_custom_html() {
        let template = format!("<style>custom</style>{DATA_BLOCK_START}old{DATA_BLOCK_END}");
        let updated = replace_data_block(
            &template,
            SpeedStatus {
                enabled: true,
                speed_enabled: true,
                multiplier: 1.5,
                countdown_ms: Some(3_890),
            },
        )
        .unwrap();
        assert!(updated.contains("<style>custom</style>"));
        assert!(updated.contains("\"multiplier\": 1.500"));
        assert!(updated.contains("\"countdown\": \"00:03\""));
    }
}
