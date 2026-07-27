use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::speed_randomizer::SpeedStatus;

const DATA_BLOCK_START: &str = r#"<script id="speed-randomizer-data" type="application/json">"#;
const DATA_BLOCK_END: &str = "</script>";
const HTML_REFRESH_SECONDS: &str = "1";
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

        let Some(updated_html) = replace_data_block(&template, status) else {
            // 用户可以自由编辑 HTML；如果删除了数据标记，DLL 不覆盖用户文件。
            self.last_write = Instant::now();
            return;
        };

        if updated_html != template {
            let _ = fs::write(&self.path, updated_html);
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
        r#"{{
  "enabled": {},
  "globalEnabled": {},
  "globalMultiplier": {:.3},
  "globalCountdown": "{}",
  "playerEnabled": {},
  "playerMultiplier": {:.3},
  "playerCountdown": "{}"
}}"#,
        status.enabled,
        status.global_enabled,
        status.global_multiplier,
        format_countdown(status.global_countdown_ms),
        status.player_enabled,
        status.player_multiplier,
        format_countdown(status.player_countdown_ms),
    )
}

fn default_template() -> String {
    format!(
        r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="{HTML_REFRESH_SECONDS}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Speed Randomizer</title>
<style>
:root {{ color-scheme: dark; font-family: "Segoe UI", "Microsoft YaHei", sans-serif; }}
body {{ margin: 0; min-width: 320px; background: #111827; color: #e5e7eb; }}
main {{ max-width: 720px; margin: 0 auto; padding: 28px 20px 36px; }}
h1 {{ margin: 0 0 8px; font-size: 28px; }}
.subtitle {{ color: #9ca3af; margin-bottom: 24px; }}
.overall {{ display: inline-block; margin-bottom: 18px; padding: 7px 12px; border-radius: 999px; background: #6b7280; color: white; font-weight: 700; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; }}
.card {{ padding: 18px; border: 1px solid #374151; border-radius: 14px; background: #1f2937; }}
.card h2 {{ margin: 0 0 14px; font-size: 19px; }}
.label {{ color: #9ca3af; font-size: 13px; }}
.value {{ margin-top: 3px; font-size: 34px; font-weight: 800; letter-spacing: .02em; }}
.countdown {{ margin-top: 14px; font-family: Consolas, monospace; font-size: 28px; color: #93c5fd; }}
.state {{ margin-top: 10px; color: #d1d5db; }}
.footer {{ margin-top: 22px; color: #6b7280; font-size: 13px; }}
</style>
</head>
<body>
<main>
<h1>速度随机</h1>
<div class="subtitle">Speed Randomizer</div>
<div class="overall" data-sr="overall-state">随机总开关：关闭</div>
<div class="grid">
<section class="card">
<h2>全局速度</h2>
<div class="label">当前倍率</div>
<div class="value" data-sr="global-multiplier">1.00x</div>
<div class="state" data-sr="global-state">状态：停用</div>
<div class="label">距离下一次随机</div>
<div class="countdown" data-sr="global-countdown">--:--</div>
</section>
<section class="card">
<h2>玩家速度</h2>
<div class="label">当前倍率</div>
<div class="value" data-sr="player-multiplier">1.00x</div>
<div class="state" data-sr="player-state">状态：停用</div>
<div class="label">距离下一次随机</div>
<div class="countdown" data-sr="player-countdown">--:--</div>
</section>
</div>
<div class="footer">倒计时格式：分钟:秒；可以直接编辑本文件的 HTML/CSS/JS。</div>
</main>

<!--
  DLL 只更新下面这个 JSON 数据块，不会覆盖其他 HTML 内容。
  如果你自定义页面，保留这个 script 的 id 和 type 即可。
  可用字段：enabled、globalEnabled、globalMultiplier、globalCountdown、
  playerEnabled、playerMultiplier、playerCountdown。
-->
<script id="speed-randomizer-data" type="application/json">
{{
  "enabled": false,
  "globalEnabled": false,
  "globalMultiplier": 1.0,
  "globalCountdown": "--:--",
  "playerEnabled": false,
  "playerMultiplier": 1.0,
  "playerCountdown": "--:--"
}}
</script>
<script>
(() => {{
  const data = JSON.parse(document.getElementById("speed-randomizer-data").textContent);
  const set = (name, value) => {{
    const element = document.querySelector(`[data-sr="${{name}}"]`);
    if (element) element.textContent = value;
  }};
  set("overall-state", `随机总开关：${{data.enabled ? "开启" : "关闭"}}`);
  set("global-multiplier", `${{Number(data.globalMultiplier).toFixed(2)}}x`);
  set("global-state", `状态：${{data.globalEnabled ? "启用" : "停用"}}`);
  set("global-countdown", data.globalCountdown);
  set("player-multiplier", `${{Number(data.playerMultiplier).toFixed(2)}}x`);
  set("player-state", `状态：${{data.playerEnabled ? "启用" : "停用"}}`);
  set("player-countdown", data.playerCountdown);
  const overall = document.querySelector('[data-sr="overall-state"]');
  if (overall) overall.style.background = data.enabled ? "#15803d" : "#6b7280";
}})();
</script>
</body>
</html>
"##,
        HTML_REFRESH_SECONDS = HTML_REFRESH_SECONDS,
    )
}

fn format_countdown(countdown_ms: Option<u64>) -> String {
    let Some(countdown_ms) = countdown_ms else {
        return "--:--".to_string();
    };

    let total_seconds = countdown_ms / 1_000;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;

    format!("{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn countdown_uses_minutes_and_seconds() {
        assert_eq!(format_countdown(Some(3_890)), "00:03");
        assert_eq!(format_countdown(Some(65_430)), "01:05");
        assert_eq!(format_countdown(None), "--:--");
    }

    #[test]
    fn data_replacement_preserves_user_html() {
        let template = r#"<style>.my-style { color: red; }</style>
<div data-sr="global-multiplier"></div>
<script id="speed-randomizer-data" type="application/json">
old data
</script>
<script>custom();</script>"#;
        let updated = replace_data_block(
            template,
            SpeedStatus {
                enabled: true,
                global_enabled: true,
                global_multiplier: 1.5,
                global_countdown_ms: Some(3_890),
                player_enabled: true,
                player_multiplier: 0.75,
                player_countdown_ms: Some(1_200),
            },
        )
        .unwrap();

        assert!(updated.contains(".my-style { color: red; }"));
        assert!(updated.contains("<script>custom();</script>"));
        assert!(!updated.contains("old data"));
        assert!(updated.contains("\"globalMultiplier\": 1.500"));
        assert!(updated.contains("\"globalCountdown\": \"00:03\""));
    }

    #[test]
    fn default_template_contains_editable_data_marker() {
        let template = default_template();
        assert!(template.contains(DATA_BLOCK_START));
        assert!(template.contains(DATA_BLOCK_END));
        assert!(template.contains("data-sr=\"global-multiplier\""));
    }
}
