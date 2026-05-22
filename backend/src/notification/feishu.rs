use serde_json::{json, Value};

use crate::notification::FeishuCardInput;

pub fn render_feishu_card(input: &FeishuCardInput) -> Value {
    let ttl_seconds = input.ttl_ms.max(0) / 1_000;
    let model = format!("{}@{}", input.model_key, input.model_version);
    let headline = format!("{} / {}", input.market_label, input.side);
    let key_line = format!(
        "Limit {} | Size {} | TTL {}s",
        input.limit_price, input.suggested_size, ttl_seconds
    );
    let confidence_line = format!(
        "Confidence {} | Window {} - {}",
        input.confidence, input.window_start, input.window_end
    );
    let analysis = format!(
        "{}\nModel {}\nChannel {} ({})\nFeatures {}",
        input.reason, model, input.channel.name, input.channel.webhook_url_masked, input.features
    );

    json!({
        "msg_type": "interactive",
        "card": {
            "config": {
                "wide_screen_mode": true
            },
            "header": {
                "template": header_template(&input.side),
                "title": {
                    "tag": "plain_text",
                    "content": format!("Polymarket {}", headline)
                }
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": format!("**{}**\n{}\n{}\n{}", headline, key_line, confidence_line, model)
                    }
                },
                {
                    "tag": "hr"
                },
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": analysis
                    }
                }
            ]
        }
    })
}

fn header_template(side: &str) -> &'static str {
    match side {
        "UP" | "Up" | "up" => "green",
        "DOWN" | "Down" | "down" => "red",
        _ => "blue",
    }
}
