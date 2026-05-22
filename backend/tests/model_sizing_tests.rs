use bigdecimal::BigDecimal;
use polymarket_backend::model::{SizingConfig, SizingEngine};
use std::str::FromStr;

#[test]
fn fractional_kelly_sizes_positive_edge_and_clamps_to_max_size() {
    let engine = SizingEngine::new(SizingConfig {
        bankroll: bd("15"),
        fraction: bd("0.5"),
        max_size: bd("1.5"),
        min_size: bd("0.5"),
    });

    let size = engine
        .suggest_size(&bd("0.60"), &bd("0.50"))
        .expect("positive edge should produce a size");

    assert_eq!(size, bd("1.5"));
}

#[test]
fn fractional_kelly_returns_none_when_edge_is_not_positive() {
    let engine = SizingEngine::default_for_bankroll(bd("15"));

    assert_eq!(engine.suggest_size(&bd("0.50"), &bd("0.55")), None);
}

#[test]
fn fractional_kelly_returns_none_when_size_is_below_minimum() {
    let engine = SizingEngine::new(SizingConfig {
        bankroll: bd("15"),
        fraction: bd("0.25"),
        max_size: bd("1.5"),
        min_size: bd("0.5"),
    });

    assert_eq!(engine.suggest_size(&bd("0.53"), &bd("0.52")), None);
}

#[test]
fn fractional_kelly_rejects_invalid_probability_or_price() {
    let engine = SizingEngine::default_for_bankroll(bd("15"));

    assert_eq!(engine.suggest_size(&bd("1.01"), &bd("0.50")), None);
    assert_eq!(engine.suggest_size(&bd("0.60"), &bd("1.00")), None);
    assert_eq!(engine.suggest_size(&bd("0.60"), &bd("0.00")), None);
}

fn bd(value: &str) -> BigDecimal {
    BigDecimal::from_str(value).expect("decimal literal should parse")
}
