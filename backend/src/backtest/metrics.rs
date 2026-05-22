use std::str::FromStr;

use bigdecimal::{BigDecimal, ToPrimitive};
use serde_json::Value;

use crate::backtest::types::{BacktestMetrics, BacktestReplayOutput, CalibrationBucket};

pub fn compute_backtest_metrics(
    model_key: &str,
    model_version: &str,
    dataset: &str,
    windows: u64,
    outputs: &[BacktestReplayOutput],
    parameters: Value,
) -> BacktestMetrics {
    let trades = outputs.len() as u64;
    let wins = outputs.iter().filter(|output| output.won).count() as u64;
    let market_key = outputs
        .first()
        .map(|output| output.market_key.clone())
        .unwrap_or_else(|| {
            dataset
                .split_once(':')
                .map_or(dataset, |(key, _)| key)
                .to_string()
        });

    BacktestMetrics {
        model_key: model_key.to_string(),
        model_version: model_version.to_string(),
        market_key,
        dataset: dataset.to_string(),
        windows,
        trades,
        wins,
        accuracy: ratio(wins, trades),
        wilson_lower_bound: wilson_lower_bound(wins, trades),
        coverage: ratio(trades, windows),
        expected_value: expected_value(outputs),
        max_drawdown: max_drawdown(outputs),
        max_consecutive_losses: max_consecutive_losses(outputs),
        average_price_paid: average_price_paid(outputs),
        calibration_buckets: calibration_buckets(outputs),
        parameters,
    }
}

fn ratio(numerator: u64, denominator: u64) -> BigDecimal {
    if denominator == 0 {
        return BigDecimal::from(0);
    }
    decimal_from_f64(numerator as f64 / denominator as f64)
}

fn expected_value(outputs: &[BacktestReplayOutput]) -> BigDecimal {
    if outputs.is_empty() {
        return BigDecimal::from(0);
    }
    let sum = outputs
        .iter()
        .map(output_profit_loss)
        .fold(BigDecimal::from(0), |total, value| total + value);
    decimal_from_f64(sum.to_f64().unwrap_or(0.0) / outputs.len() as f64)
}

fn output_profit_loss(output: &BacktestReplayOutput) -> BigDecimal {
    let stake = &output.alert.limit_price * &output.alert.suggested_size;
    if output.won {
        output.alert.suggested_size.clone() - stake
    } else {
        -stake
    }
}

fn max_drawdown(outputs: &[BacktestReplayOutput]) -> BigDecimal {
    let mut equity = BigDecimal::from(0);
    let mut peak = BigDecimal::from(0);
    let mut max_drawdown = BigDecimal::from(0);

    for output in outputs {
        equity += output_profit_loss(output);
        if equity > peak {
            peak = equity.clone();
        }
        let drawdown = &peak - &equity;
        if drawdown > max_drawdown {
            max_drawdown = drawdown;
        }
    }

    decimal_from_f64(max_drawdown.to_f64().unwrap_or(0.0))
}

fn max_consecutive_losses(outputs: &[BacktestReplayOutput]) -> u64 {
    let mut current = 0;
    let mut max_seen = 0;
    for output in outputs {
        if output.won {
            current = 0;
        } else {
            current += 1;
            max_seen = max_seen.max(current);
        }
    }
    max_seen
}

fn average_price_paid(outputs: &[BacktestReplayOutput]) -> BigDecimal {
    if outputs.is_empty() {
        return BigDecimal::from(0);
    }
    let sum = outputs
        .iter()
        .filter_map(|output| output.alert.limit_price.to_f64())
        .sum::<f64>();
    decimal_from_f64(sum / outputs.len() as f64)
}

fn calibration_buckets(outputs: &[BacktestReplayOutput]) -> Vec<CalibrationBucket> {
    let bucket_ranges = [(0.5, 0.6), (0.6, 0.7), (0.7, 0.8), (0.8, 0.9), (0.9, 1.0)];
    let mut buckets = Vec::new();

    for (lower, upper) in bucket_ranges {
        let matching = outputs
            .iter()
            .filter(|output| {
                let confidence = output.alert.confidence.to_f64().unwrap_or(0.0);
                confidence >= lower && (confidence < upper || (upper == 1.0 && confidence <= upper))
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            continue;
        }
        let trades = matching.len() as u64;
        let wins = matching.iter().filter(|output| output.won).count() as u64;
        buckets.push(CalibrationBucket {
            lower_bound: decimal_from_f64(lower),
            upper_bound: decimal_from_f64(upper),
            trades,
            wins,
            accuracy: ratio(wins, trades),
        });
    }

    buckets
}

fn wilson_lower_bound(wins: u64, trades: u64) -> BigDecimal {
    if trades == 0 {
        return BigDecimal::from(0);
    }
    let z = 1.96_f64;
    let n = trades as f64;
    let p = wins as f64 / n;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let centre = p + z2 / (2.0 * n);
    let margin = z * ((p * (1.0 - p) + z2 / (4.0 * n)) / n).sqrt();
    decimal_from_f64((centre - margin) / denominator)
}

fn decimal_from_f64(value: f64) -> BigDecimal {
    if !value.is_finite() {
        return BigDecimal::from(0);
    }
    let mut text = format!("{value:.9}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".to_string();
    }
    BigDecimal::from_str(&text).expect("formatted decimal should parse")
}
