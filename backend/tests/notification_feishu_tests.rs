use polymarket_backend::notification::{
    render_feishu_card, FeishuCardInput, NotificationChannelView,
};
use time::{Duration, OffsetDateTime};

#[test]
fn feishu_card_first_screen_contains_actionable_signal_text() {
    let card = render_feishu_card(&input());
    let text = serde_json::to_string(&card).expect("card should serialize");

    assert_eq!(card["msg_type"], "interactive");
    assert!(text.contains("BTC 5m"));
    assert!(text.contains("UP"));
    assert!(text.contains("Limit 0.52"));
    assert!(text.contains("Size 1.50"));
    assert!(text.contains("TTL 15s"));
    assert!(text.contains("Confidence 0.68"));
    assert!(text.contains("baseline_direction@0.1.0"));
}

#[test]
fn feishu_card_keeps_analysis_after_key_fields() {
    let card = render_feishu_card(&input());
    let elements = card["card"]["elements"]
        .as_array()
        .expect("card elements should be an array");
    let first_text = elements[0].to_string();
    let last_text = elements.last().expect("last element").to_string();

    assert!(first_text.contains("Limit 0.52"));
    assert!(first_text.contains("Size 1.50"));
    assert!(last_text.contains("directional threshold crossed"));
}

#[test]
fn feishu_card_payload_never_contains_webhook_url() {
    let card = render_feishu_card(&input());
    let text = serde_json::to_string(&card).expect("card should serialize");

    assert!(!text.contains("secret-token"));
    assert!(text.contains("primary"));
    assert!(text.contains(".../oken"));
}

fn input() -> FeishuCardInput {
    FeishuCardInput {
        market_key: "btc5m".to_string(),
        market_label: "BTC 5m".to_string(),
        window_start: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_800),
        window_end: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_100),
        side: "UP".to_string(),
        confidence: "0.68".to_string(),
        limit_price: "0.52".to_string(),
        suggested_size: "1.50".to_string(),
        ttl_ms: 15_000,
        model_key: "baseline_direction".to_string(),
        model_version: "0.1.0".to_string(),
        reason: "directional threshold crossed".to_string(),
        features: serde_json::json!({
            "return_bps": "8.4",
            "threshold_bps": "4"
        }),
        channel: NotificationChannelView {
            name: "primary".to_string(),
            webhook_url_masked: "https://open.feishu.cn/.../oken".to_string(),
        },
    }
}
