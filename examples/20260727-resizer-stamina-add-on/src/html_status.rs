use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use crate::resizer_stamina::ResizerStaminaStatus;

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

    pub fn write_if_due(&mut self, status: ResizerStaminaStatus) {
        if self.last_write.elapsed() < MIN_WRITE_INTERVAL {
            return;
        }

        let _ = fs::write(&self.path, render_page(status));
        self.last_write = Instant::now();
    }
}

fn render_page(status: ResizerStaminaStatus) -> String {
    let overall_text = if status.player_available {
        "运行中"
    } else {
        "等待玩家数据"
    };
    let overall_color = if status.player_available {
        "#15803d"
    } else {
        "#6b7280"
    };
    let stamina_value = if status.player_available {
        format!("{} / {}", status.current_stamina, status.max_stamina)
    } else {
        "-- / --".to_string()
    };
    let stamina_percent = if status.player_available {
        format!("{:.2}%", status.stamina_percent)
    } else {
        "--".to_string()
    };
    let written_scale = if status.player_available {
        format!("{}%", status.written_scale_percent)
    } else {
        "--".to_string()
    };
    let scale_multiplier = if status.player_available {
        format!("{:.3}x", status.written_scale_percent as f32 / 100.0)
    } else {
        "--".to_string()
    };
    let calculated_scale = if status.player_available {
        format!("{:.2}%", status.calculated_scale_percent)
    } else {
        "--".to_string()
    };
    let base_scale = if status.player_available {
        format!("{:.2}%", status.base_scale_percent)
    } else {
        "--".to_string()
    };
    let capacity_factor = if status.player_available {
        format!("{:.3}x", status.capacity_factor)
    } else {
        "--".to_string()
    };
    let resizer_state = if !status.resizer_config_found {
        "未找到 Resizer_config.ini"
    } else if status.last_write_succeeded {
        "已同步 playerScale"
    } else {
        "写入失败"
    };
    let resizer_color = if status.last_write_succeeded {
        "#86efac"
    } else {
        "#fca5a5"
    };

    format!(
        r##"<!doctype html>
<html lang="zh-CN">
<head>
<meta charset="utf-8">
<meta http-equiv="refresh" content="{HTML_REFRESH_SECONDS}">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Resizer Stamina Add-on</title>
<style>
:root {{ color-scheme: dark; font-family: "Segoe UI", "Microsoft YaHei", sans-serif; }}
body {{ margin: 0; min-width: 320px; background: #111827; color: #e5e7eb; }}
main {{ max-width: 720px; margin: 0 auto; padding: 28px 20px 36px; }}
h1 {{ margin: 0 0 8px; font-size: 28px; }}
.subtitle {{ color: #9ca3af; margin-bottom: 24px; }}
.overall {{ display: inline-block; margin-bottom: 18px; padding: 7px 12px; border-radius: 999px; background: {overall_color}; color: white; font-weight: 700; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; }}
.card {{ padding: 18px; border: 1px solid #374151; border-radius: 14px; background: #1f2937; }}
.card h2 {{ margin: 0 0 14px; font-size: 19px; }}
.label {{ color: #9ca3af; font-size: 13px; }}
.value {{ margin-top: 3px; font-size: 34px; font-weight: 800; letter-spacing: .02em; }}
.detail {{ margin-top: 10px; color: #d1d5db; }}
.detail strong {{ color: #f3f4f6; }}
.state {{ margin-top: 10px; color: {resizer_color}; }}
.footer {{ margin-top: 22px; color: #6b7280; font-size: 13px; line-height: 1.6; }}
</style>
</head>
<body>
<main>
<h1>耐力体型联动</h1>
<div class="subtitle">Resizer Stamina Add-on</div>
<div class="overall">状态：{overall_text}</div>
<div class="grid">
<section class="card">
<h2>玩家绿条</h2>
<div class="label">当前 / 上限</div>
<div class="value">{stamina_value}</div>
<div class="detail">当前百分比：<strong>{stamina_percent}</strong></div>
<div class="detail">base_stamina：<strong>{base_stamina}</strong></div>
<div class="detail">上限修正：<strong>{capacity_factor}</strong></div>
</section>
<section class="card">
<h2>玩家体型</h2>
<div class="label">写入 Resizer 的体型</div>
<div class="value">{written_scale}</div>
<div class="detail">当前倍率：<strong>{scale_multiplier}</strong></div>
<div class="detail">公式计算值：<strong>{calculated_scale}</strong></div>
<div class="detail">上限修正前：<strong>{base_scale}</strong></div>
</section>
<section class="card">
<h2>同步状态</h2>
<div class="label">刷新频率</div>
<div class="value">{update_interval_frames} 帧</div>
<div class="state">{resizer_state}</div>
<div class="detail">目标文件：<strong>Resizer_config.ini</strong></div>
</section>
</div>
<div class="footer">
体型公式：(最小体型 + 当前耐力 / 耐力上限 × (最大体型 - 最小体型)) × (耐力上限 / base_stamina)。<br>
页面每秒自动刷新，仅用于状态展示；所有配置请修改 DLL 同目录下的 resizer_stamina_add_on.toml。
</div>
</main>
</body>
</html>
"##,
        HTML_REFRESH_SECONDS = HTML_REFRESH_SECONDS,
        overall_color = overall_color,
        overall_text = overall_text,
        stamina_value = stamina_value,
        stamina_percent = stamina_percent,
        base_stamina = status.base_stamina,
        capacity_factor = capacity_factor,
        written_scale = written_scale,
        scale_multiplier = scale_multiplier,
        calculated_scale = calculated_scale,
        base_scale = base_scale,
        update_interval_frames = status.update_interval_frames,
        resizer_color = resizer_color,
        resizer_state = resizer_state,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_status() -> ResizerStaminaStatus {
        ResizerStaminaStatus {
            player_available: true,
            current_stamina: 150,
            max_stamina: 300,
            stamina_percent: 50.0,
            base_stamina: 200,
            capacity_factor: 1.5,
            base_scale_percent: 155.0,
            calculated_scale_percent: 232.5,
            written_scale_percent: 233,
            update_interval_frames: 5,
            resizer_config_found: true,
            last_write_succeeded: true,
        }
    }

    #[test]
    fn page_uses_speed_randomizer_style_and_shows_status() {
        let page = render_page(sample_status());
        assert!(page.contains("background: #111827"));
        assert!(page.contains("background: #1f2937"));
        assert!(page.contains("150 / 300"));
        assert!(page.contains("50.00%"));
        assert!(page.contains("233%"));
        assert!(page.contains("232.50%"));
        assert!(page.contains("5 帧"));
    }

    #[test]
    fn page_has_no_controls() {
        let page = render_page(sample_status());
        assert!(!page.contains("<input"));
        assert!(!page.contains("<button"));
        assert!(!page.contains("<form"));
    }
}
