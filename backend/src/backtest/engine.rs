use bigdecimal::BigDecimal;
use serde_json::Value;

use crate::backtest::{
    compute_backtest_metrics, BacktestReplayOutput, BacktestReplayResult, FrozenActionableAlert,
    ReplayWindow, ReplayWindowCandidate,
};

pub fn replay_backtest(
    model_key: &str,
    model_version: &str,
    dataset: &str,
    parameters: Value,
    windows: &[ReplayWindow],
) -> BacktestReplayResult {
    let replay_outputs = windows
        .iter()
        .filter_map(first_frozen_output)
        .collect::<Vec<_>>();
    let metrics = compute_backtest_metrics(
        model_key,
        model_version,
        dataset,
        windows.len() as u64,
        &replay_outputs,
        parameters,
    );

    BacktestReplayResult {
        metrics,
        replay_outputs,
    }
}

fn first_frozen_output(window: &ReplayWindow) -> Option<BacktestReplayOutput> {
    let candidate = window
        .candidates
        .iter()
        .min_by_key(|candidate| candidate.created_at)?;
    let won = candidate_won(candidate, window);
    let profit_loss = profit_loss(candidate, won);

    Some(BacktestReplayOutput {
        market_key: window.market_key.clone(),
        window_start: window.window_start,
        window_end: window.window_end,
        alert: FrozenActionableAlert {
            created_at: candidate.created_at,
            side: candidate.side.clone(),
            confidence: candidate.confidence.clone(),
            limit_price: candidate.limit_price.clone(),
            suggested_size: candidate.suggested_size.clone(),
            ttl_ms: candidate.ttl_ms,
            reason: candidate.reason.clone(),
            features: candidate.features.clone(),
            input_snapshot_hash: candidate.input_snapshot_hash.clone(),
        },
        open_price: window.open_price.clone(),
        final_price: window.final_price.clone(),
        won,
        profit_loss,
    })
}

fn candidate_won(candidate: &ReplayWindowCandidate, window: &ReplayWindow) -> bool {
    match candidate.side.as_str() {
        "Up" => window.final_price > window.open_price,
        "Down" => window.final_price < window.open_price,
        _ => false,
    }
}

fn profit_loss(candidate: &ReplayWindowCandidate, won: bool) -> BigDecimal {
    let stake = &candidate.limit_price * &candidate.suggested_size;
    if won {
        candidate.suggested_size.clone() - stake
    } else {
        -stake
    }
}
