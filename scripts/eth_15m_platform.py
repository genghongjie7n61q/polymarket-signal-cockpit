#!/usr/bin/env python3
import argparse
import asyncio
import dataclasses
import datetime as dt
import json
import math
import os
import re
import subprocess
import statistics
import time
from typing import Optional
import urllib.parse
import urllib.request


UA = {"User-Agent": "curl/8.7.1", "Accept": "application/json"}
DEFAULT_SLUG = "eth-updown-15m-1779324300"
COINBASE_WS_URL = "wss://ws-feed.exchange.coinbase.com"


def fetch_json(base, params=None, timeout=15):
    url = base + (("?" + urllib.parse.urlencode(params)) if params else "")
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def parse_json_array(value):
    if isinstance(value, list):
        return value
    if not value:
        return []
    return json.loads(value)


def parse_int_csv(value):
    if not value:
        return []
    if isinstance(value, (list, tuple)):
        return [int(item) for item in value]
    return [int(item.strip()) for item in str(value).split(",") if item.strip()]


def iso_to_dt(value):
    return dt.datetime.fromisoformat(value.replace("Z", "+00:00"))


def ts_to_iso(timestamp):
    return dt.datetime.fromtimestamp(timestamp, dt.timezone.utc).isoformat().replace(
        "+00:00", "Z"
    )


def floor_to_15m(timestamp):
    return timestamp - (timestamp % (15 * 60))


def get_event(slug):
    return fetch_json(f"https://gamma-api.polymarket.com/events/slug/{slug}")


def get_current_eth_15m_event(now_ts=None):
    now_ts = now_ts or int(dt.datetime.now(dt.timezone.utc).timestamp())
    current_start = floor_to_15m(now_ts)
    # Try current, previous, and next windows. Gamma search can lag or return stale
    # active markets, while event slugs are timestamped by interval start.
    for start_ts in (current_start, current_start - 15 * 60, current_start + 15 * 60):
        slug = f"eth-updown-15m-{start_ts}"
        try:
            event = get_event(slug)
        except Exception:
            continue
        market = event_market(event)
        try:
            start = iso_to_dt(market["endDate"]).timestamp() - 15 * 60
            end = iso_to_dt(market["endDate"]).timestamp()
        except (KeyError, TypeError, ValueError):
            continue
        if start <= now_ts < end:
            return event
    # If no live window is available, return the next one if it exists.
    return get_event(f"eth-updown-15m-{current_start + 15 * 60}")


def event_market(event):
    markets = event.get("markets") or []
    if not markets:
        raise RuntimeError(f"No markets found for event {event.get('slug')}")
    return markets[0]


def market_snapshot(event):
    market = event_market(event)
    outcomes = parse_json_array(market.get("outcomes"))
    prices = [float(price) for price in parse_json_array(market.get("outcomePrices"))]
    price_by_outcome = dict(zip(outcomes, prices))
    return {
        "event_id": event.get("id"),
        "event_slug": event.get("slug"),
        "title": event.get("title"),
        "description": event.get("description"),
        "market_id": market.get("id"),
        "market_slug": market.get("slug"),
        "start_at": market.get("startDate"),
        "end_at": market.get("endDate"),
        "outcomes": outcomes,
        "prices": price_by_outcome,
        "best_bid": market.get("bestBid"),
        "best_ask": market.get("bestAsk"),
        "spread": float(market.get("spread") or 0),
        "volume24hr": float(market.get("volume24hr") or 0),
        "liquidity": float(market.get("liquidityNum") or 0),
        "active": market.get("active"),
        "closed": market.get("closed"),
    }


def coinbase_candles(start_ts, end_ts, granularity=60, product_id="ETH-USD"):
    candles = []
    cursor = int(start_ts)
    # Coinbase returns at most roughly 300 candles per request.
    step = granularity * 300
    while cursor < end_ts:
        chunk_end = min(cursor + step, int(end_ts))
        params = {
            "start": ts_to_iso(cursor),
            "end": ts_to_iso(chunk_end),
            "granularity": granularity,
        }
        chunk = fetch_json(
            f"https://api.exchange.coinbase.com/products/{product_id}/candles",
            params,
            timeout=20,
        )
        candles.extend(chunk)
        cursor = chunk_end
        time.sleep(0.05)
    # [time, low, high, open, close, volume]
    unique = {int(row[0]): row for row in candles}
    return [unique[key] for key in sorted(unique)]


def candle_maps(candles):
    return {
        int(row[0]): {
            "low": float(row[1]),
            "high": float(row[2]),
            "open": float(row[3]),
            "close": float(row[4]),
            "volume": float(row[5]),
        }
        for row in candles
    }


def price_from_candle(candles_by_ts, timestamp, field):
    row = candles_by_ts.get(int(timestamp))
    if not row:
        return None
    return row[field]


def price_at_or_before(candles_by_ts, timestamp, field):
    available = [ts for ts in candles_by_ts if ts <= int(timestamp)]
    if not available:
        return None
    return candles_by_ts[max(available)][field]


def pct_return(new, old):
    if old is None or new is None or old == 0:
        return None
    return (new - old) / old


def wilson_lower_bound(wins, trials, z=1.96):
    if trials <= 0:
        return 0
    p_hat = wins / trials
    denominator = 1 + z * z / trials
    centre = p_hat + z * z / (2 * trials)
    margin = z * math.sqrt((p_hat * (1 - p_hat) + z * z / (4 * trials)) / trials)
    return (centre - margin) / denominator


def kelly_stake(bankroll, probability, price, fraction=0.10, cap_fraction=0.10):
    if price <= 0 or price >= 1:
        raise ValueError("price must be between 0 and 1")
    full_fraction = max(0, (probability - price) / (1 - price))
    suggested_fraction = min(full_fraction * fraction, cap_fraction)
    suggested_stake = bankroll * suggested_fraction
    return {
        "bankroll": bankroll,
        "probability": probability,
        "price": price,
        "full_kelly_fraction": round(full_fraction, 6),
        "suggested_fraction": round(suggested_fraction, 6),
        "suggested_stake": round(suggested_stake, 2),
    }


def policy_signal(pick, entry_price, fair_price_after_margin, spread, min_edge=0.03):
    edge = fair_price_after_margin - entry_price
    action = "RESEARCH_CANDIDATE" if edge >= min_edge and spread <= 0.03 else "NO_TRADE"
    return {
        "pick": pick,
        "entry_price": entry_price,
        "model_price_after_margin": fair_price_after_margin,
        "edge_after_margin": round(edge, 4),
        "action": action,
    }


@dataclasses.dataclass
class RealtimeWindowState:
    start_ts: int
    end_ts: int
    open_price: Optional[float] = None
    latest_price: Optional[float] = None
    latest_ts: Optional[int] = None

    def update(self, timestamp, price):
        if timestamp < self.start_ts or timestamp >= self.end_ts:
            return
        if self.open_price is None:
            self.open_price = price
        self.latest_price = price
        self.latest_ts = timestamp

    @property
    def elapsed_minutes(self):
        if self.latest_ts is None:
            return 0
        return max(0, math.floor((self.latest_ts - self.start_ts) / 60))

    @property
    def decision_return(self):
        return pct_return(self.latest_price, self.open_price)


@dataclasses.dataclass(frozen=True)
class MarketConfig:
    key: str
    asset_label: str
    product_id: str
    slug_prefix: str
    interval_minutes: int
    default_windows: int
    default_decision_minutes: list
    default_threshold_bps_grid: list


MARKET_CONFIGS = {
    "eth15m": MarketConfig(
        key="eth15m",
        asset_label="ETH",
        product_id="ETH-USD",
        slug_prefix="eth-updown-15m",
        interval_minutes=15,
        default_windows=192,
        default_decision_minutes=[3, 4, 5, 6, 7],
        default_threshold_bps_grid=[4, 6, 8, 10, 12],
    ),
    "btc5m": MarketConfig(
        key="btc5m",
        asset_label="BTC",
        product_id="BTC-USD",
        slug_prefix="btc-updown-5m",
        interval_minutes=5,
        default_windows=576,
        default_decision_minutes=[1, 2, 3],
        default_threshold_bps_grid=[2, 4, 6, 8, 10],
    ),
}


def market_config(key):
    try:
        return MARKET_CONFIGS[key]
    except KeyError as exc:
        raise ValueError(f"Unsupported market config: {key}") from exc


def floor_to_interval(timestamp, interval_minutes):
    interval_seconds = interval_minutes * 60
    return timestamp - (timestamp % interval_seconds)


def build_event_slug(config, timestamp):
    return f"{config.slug_prefix}-{floor_to_interval(timestamp, config.interval_minutes)}"


def get_current_event(config, now_ts=None):
    now_ts = now_ts or int(dt.datetime.now(dt.timezone.utc).timestamp())
    current_start = floor_to_interval(now_ts, config.interval_minutes)
    interval_seconds = config.interval_minutes * 60
    # Gamma search can lag, so probe adjacent timestamped markets.
    for start_ts in (current_start, current_start - interval_seconds, current_start + interval_seconds):
        try:
            event = get_event(f"{config.slug_prefix}-{start_ts}")
        except Exception:
            continue
        market = event_market(event)
        try:
            start = iso_to_dt(market["endDate"]).timestamp() - interval_seconds
            end = iso_to_dt(market["endDate"]).timestamp()
        except (KeyError, TypeError, ValueError):
            continue
        if start <= now_ts < end:
            return event
    return get_event(f"{config.slug_prefix}-{current_start + interval_seconds}")


def get_current_eth_15m_event(now_ts=None):
    return get_current_event(market_config("eth15m"), now_ts)


def build_window_records(candles_by_ts, end_ts, windows, decision_minute, interval_minutes=15):
    records = []
    interval_seconds = interval_minutes * 60
    current_start = floor_to_interval(end_ts, interval_minutes) - interval_seconds
    earliest_start = current_start - windows * interval_seconds
    for start in range(earliest_start, current_start, interval_seconds):
        start_open = price_from_candle(candles_by_ts, start, "open")
        end_close = price_from_candle(
            candles_by_ts, start + (interval_minutes - 1) * 60, "close"
        )
        decision_close = price_from_candle(
            candles_by_ts, start + (decision_minute - 1) * 60, "close"
        )
        pre5_open = price_from_candle(candles_by_ts, start - 5 * 60, "open")
        if None in (start_open, end_close, decision_close, pre5_open):
            continue
        actual_up = end_close >= start_open
        records.append(
            {
                "start_ts": start,
                "start_iso": ts_to_iso(start),
                "start_price": start_open,
                "end_price": end_close,
                "decision_price": decision_close,
                "actual": "Up" if actual_up else "Down",
                "pre5_return": pct_return(start_open, pre5_open),
                "decision_return": pct_return(decision_close, start_open),
            }
        )
    return records


def summarize_strategy(records, name, chooser):
    trades = []
    for record in records:
        pick = chooser(record)
        if not pick:
            continue
        won = pick == record["actual"]
        trades.append({"pick": pick, "won": won})
    if not trades:
        return {"name": name, "trades": 0}
    wins = sum(1 for trade in trades if trade["won"])
    accuracy = wins / len(trades)
    return {
        "name": name,
        "trades": len(trades),
        "coverage": round(len(trades) / len(records), 4) if records else 0,
        "wins": wins,
        "accuracy": round(accuracy, 4),
        "win_rate_lower_bound": round(wilson_lower_bound(wins, len(trades)), 4),
        "break_even_price": round(accuracy, 4),
        "fair_price_with_3pct_margin": round(max(0, accuracy - 0.03), 4),
    }


def backtest(records, decision_minute, threshold_bps_values):
    if isinstance(threshold_bps_values, int):
        threshold_bps_values = [threshold_bps_values]
    rows = [
        summarize_strategy(
            records,
            f"early_direction_m{decision_minute}",
            lambda row: "Up" if row["decision_return"] >= 0 else "Down",
        ),
        summarize_strategy(
            records,
            "pre5_momentum",
            lambda row: "Up" if row["pre5_return"] >= 0 else "Down",
        ),
    ]
    for threshold_bps in threshold_bps_values:
        threshold = threshold_bps / 10000
        rows.append(
            summarize_strategy(
                records,
                f"early_direction_threshold_m{decision_minute}_{threshold_bps}bps",
                lambda row, threshold=threshold: (
                    "Up"
                    if row["decision_return"] >= threshold
                    else ("Down" if row["decision_return"] <= -threshold else None)
                ),
            )
        )
    return rows


def evaluate_backtests(
    candles_by_ts,
    end_ts,
    windows,
    decision_minutes,
    threshold_bps_values,
    interval_minutes=15,
):
    records_by_minute = {}
    rows = []
    for decision_minute in decision_minutes:
        records = build_window_records(
            candles_by_ts,
            end_ts,
            windows,
            decision_minute,
            interval_minutes=interval_minutes,
        )
        records_by_minute[decision_minute] = records
        rows.extend(backtest(records, decision_minute, threshold_bps_values))
    return rows, records_by_minute


def choose_strategy(backtest_rows):
    candidates = [
        {
            **row,
            "win_rate_lower_bound": row.get("win_rate_lower_bound")
            or round(wilson_lower_bound(row.get("wins", 0), row.get("trades", 0)), 4),
        }
        for row in backtest_rows
        if row.get("trades", 0) >= 20 and row.get("coverage", 0) >= 0.2
    ]
    if not candidates:
        return None
    return max(
        candidates,
        key=lambda row: (
            row["win_rate_lower_bound"],
            row["accuracy"],
            row["trades"],
        ),
    )


def pick_from_strategy(strategy, decision_return=None, pre5_return=None):
    if not strategy:
        return None, "No selected strategy."
    name = strategy.get("name", "")
    if name == "pre5_momentum":
        if pre5_return is None:
            return None, "Waiting for pre-start momentum data."
        return ("Up" if pre5_return >= 0 else "Down"), "pre5_momentum"

    if name.startswith("early_direction_threshold_"):
        if decision_return is None:
            return None, "Waiting for live decision return."
        match = re.search(r"threshold_(?:m\d+_)?(\d+)bps", name)
        threshold_bps = int(match.group(1)) if match else 0
        threshold = threshold_bps / 10000
        if decision_return >= threshold:
            return "Up", f"decision return is above {threshold_bps}bps threshold"
        if decision_return <= -threshold:
            return "Down", f"decision return is below -{threshold_bps}bps threshold"
        return None, f"Waiting until move clears {threshold_bps}bps threshold."

    if name.startswith("early_direction_"):
        if decision_return is None:
            return None, "Waiting for live decision return."
        return ("Up" if decision_return >= 0 else "Down"), "early_direction"

    return None, f"Unsupported strategy: {name}"


def strategy_decision_minute(strategy, default):
    if not strategy:
        return default
    match = re.search(r"_m(\d+)", strategy.get("name", ""))
    return int(match.group(1)) if match else default


def current_signal(snapshot, candles_by_ts, decision_minute, strategy, config=None):
    config = config or market_config("eth15m")
    now_ts = int(dt.datetime.now(dt.timezone.utc).timestamp())
    interval_seconds = config.interval_minutes * 60
    start_ts = int(iso_to_dt(snapshot["end_at"]).timestamp()) - interval_seconds
    end_ts = int(iso_to_dt(snapshot["end_at"]).timestamp())
    start_open = price_from_candle(candles_by_ts, start_ts, "open")
    elapsed_minutes = math.floor((now_ts - start_ts) / 60)
    current_minute_ts = min(start_ts + elapsed_minutes * 60, end_ts - 60)
    current_close = price_at_or_before(candles_by_ts, current_minute_ts, "close")
    pre5_open = price_from_candle(candles_by_ts, start_ts - 5 * 60, "open")
    if now_ts < start_ts:
        phase = "pre_start"
    elif now_ts >= end_ts:
        phase = "ended_or_settling"
    else:
        phase = "live"
    decision_return = pct_return(current_close, start_open)
    pre5_return = pct_return(start_open, pre5_open)

    pick = None
    reason = "No usable strategy or market phase."
    if phase == "live" and elapsed_minutes >= decision_minute and decision_return is not None:
        pick, strategy_reason = pick_from_strategy(
            strategy, decision_return=decision_return, pre5_return=pre5_return
        )
        reason = (
            f"Live interval minute {elapsed_minutes}; {config.asset_label} is "
            f"{decision_return * 100:.3f}% from interval open. {strategy_reason}"
        )
    elif phase == "pre_start" and pre5_return is not None:
        pick, strategy_reason = pick_from_strategy(
            strategy, decision_return=decision_return, pre5_return=pre5_return
        )
        reason = f"Pre-start 5m momentum is {pre5_return * 100:.3f}%. {strategy_reason}"

    if not pick or not strategy:
        return {
            "phase": phase,
            "pick": None,
            "action": "NO_TRADE",
            "reason": reason,
            "elapsed_minutes": elapsed_minutes,
            "decision_return": decision_return,
            "pre5_return": pre5_return,
        }

    entry_price = snapshot["prices"].get(pick)
    model_price = strategy["fair_price_with_3pct_margin"]
    if entry_price is None:
        return {
            "phase": phase,
            "pick": pick,
            "action": "NO_TRADE",
            "reason": reason,
            "elapsed_minutes": elapsed_minutes,
            "decision_return": decision_return,
            "pre5_return": pre5_return,
        }
    policy = policy_signal(pick, entry_price, model_price, snapshot["spread"])
    action = policy["action"]
    if snapshot["spread"] > 0.03:
        action = "NO_TRADE"
        reason += " Spread is too wide."
    return {
        "phase": phase,
        "pick": pick,
        "entry_price": entry_price,
        "model_price_after_margin": model_price,
        "edge_after_margin": policy["edge_after_margin"],
        "action": action,
        "reason": reason,
        "elapsed_minutes": elapsed_minutes,
        "decision_return": decision_return,
        "pre5_return": pre5_return,
    }


def realtime_signal_from_state(snapshot, state, decision_minute, strategy, bankroll, asset_label="ETH"):
    if not strategy or state.elapsed_minutes < decision_minute or state.decision_return is None:
        return {
            "phase": "live",
            "pick": None,
            "action": "NO_TRADE",
            "reason": "Waiting for decision minute and selected strategy.",
            "elapsed_minutes": state.elapsed_minutes,
            "decision_return": state.decision_return,
        }
    pick, strategy_reason = pick_from_strategy(
        strategy, decision_return=state.decision_return
    )
    if not pick:
        return {
            "phase": "live",
            "pick": None,
            "action": "NO_TRADE",
            "reason": strategy_reason,
            "elapsed_minutes": state.elapsed_minutes,
            "decision_return": state.decision_return,
        }
    entry_price = snapshot["prices"].get(pick)
    if entry_price is None:
        return {
            "phase": "live",
            "pick": pick,
            "action": "NO_TRADE",
            "reason": "Missing entry price.",
        }
    policy = policy_signal(
        pick,
        entry_price,
        strategy["fair_price_with_3pct_margin"],
        snapshot["spread"],
    )
    stake = kelly_stake(bankroll, strategy["accuracy"], entry_price)
    policy.update(
        {
            "phase": "live",
            "reason": (
                f"WebSocket minute {state.elapsed_minutes}; {asset_label} is "
                f"{state.decision_return * 100:.3f}% from interval open. "
                f"{strategy_reason}"
            ),
            "elapsed_minutes": state.elapsed_minutes,
            "decision_return": state.decision_return,
            "stake": stake,
            "paper_order": {
                "side": pick,
                "limit_price": entry_price,
                "suggested_size_usd": stake["suggested_stake"],
                "mode": "paper_only",
            },
        }
    )
    return policy


def write_alert(path, payload):
    if not path:
        return
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(json.dumps(payload, ensure_ascii=False) + "\n")


def write_log_event(path, event, **fields):
    if not path:
        return
    row = {
        "ts_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "event": event,
        **fields,
    }
    with open(path, "a", encoding="utf-8") as handle:
        handle.write(json.dumps(row, ensure_ascii=False) + "\n")


def should_log_tick(last_logged_minute, signal):
    if signal.get("action") == "RESEARCH_CANDIDATE":
        return True
    minute = signal.get("elapsed_minutes")
    return minute is not None and minute != last_logged_minute


def normalize_monitor_args(args):
    if getattr(args, "loop", False):
        args.auto_current = True
        args.slug = ""
    return args


def run_alert_command(command, payload):
    if not command:
        return
    subprocess.run(
        command,
        input=json.dumps(payload, ensure_ascii=False),
        text=True,
        shell=True,
        check=False,
    )


def fmt_money(value):
    if value is None:
        return "N/A"
    return f"${float(value):.2f}"


def fmt_price(value):
    if value is None:
        return "N/A"
    return f"{float(value):.3f}".rstrip("0").rstrip(".")


def fmt_pct(value):
    if value is None:
        return "N/A"
    return f"{float(value) * 100:.2f}%"


def build_feishu_payload(payload):
    signal = payload.get("signal") or {}
    market = payload.get("market") or {}
    backtest = payload.get("backtest") or {}
    strategy = backtest.get("selected_strategy") or {}
    paper_order = signal.get("paper_order") or {}
    label = payload.get("label") or "Polymarket 信号"
    pick = signal.get("pick") or paper_order.get("side") or "N/A"
    limit_price = paper_order.get("limit_price", signal.get("entry_price"))
    suggested_size = paper_order.get("suggested_size_usd")

    primary = "\n".join(
        [
            f"**方向: {pick}**",
            f"**限价: {fmt_price(limit_price)}**",
            f"**建议金额: {fmt_money(suggested_size)}**",
            f"市场: {market.get('title', 'N/A')}",
        ]
    )
    details = "\n".join(
        [
            "",
            "---",
            f"策略: {strategy.get('name', 'N/A')}",
            f"回测胜率: {fmt_pct(strategy.get('accuracy'))}",
            f"Wilson下界: {fmt_pct(strategy.get('win_rate_lower_bound'))}",
            f"样本数: {strategy.get('trades', 'N/A')}",
            f"边际优势: {fmt_pct(signal.get('edge_after_margin'))}",
            f"原因: {signal.get('reason', 'N/A')}",
            "模式: 研究信号 / 手动确认 / 不自动下单",
        ]
    )
    return {
        "msg_type": "interactive",
        "card": {
            "config": {"wide_screen_mode": True},
            "header": {
                "template": "red" if pick == "Down" else "green",
                "title": {"tag": "plain_text", "content": label},
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {"tag": "lark_md", "content": primary + details},
                }
            ],
        },
    }


def send_feishu_alert(webhook_url, payload, timeout=10):
    if not webhook_url:
        return None
    body = json.dumps(build_feishu_payload(payload), ensure_ascii=False).encode("utf-8")
    request = urllib.request.Request(
        webhook_url,
        data=body,
        headers={"Content-Type": "application/json; charset=utf-8"},
        method="POST",
    )
    with urllib.request.urlopen(request, timeout=timeout) as response:
        raw = response.read().decode("utf-8")
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return {"raw": raw}


async def monitor_websocket(args):
    try:
        import websockets
    except ImportError as exc:
        raise RuntimeError(
            "The websocket monitor requires the 'websockets' Python package. "
            "Install it with: python3 -m pip install websockets"
        ) from exc

    config = market_config(args.market)
    event = get_current_event(config) if args.auto_current or not args.slug else get_event(args.slug)
    snapshot = market_snapshot(event)
    interval_seconds = config.interval_minutes * 60
    start_ts = int(iso_to_dt(snapshot["end_at"]).timestamp()) - interval_seconds
    end_ts = int(iso_to_dt(snapshot["end_at"]).timestamp())
    decision_minutes = (
        parse_int_csv(args.decision_minutes)
        or ([args.decision_minute] if args.decision_minute is not None else config.default_decision_minutes)
    )
    threshold_grid = (
        parse_int_csv(args.threshold_bps_grid)
        or ([args.threshold_bps] if args.threshold_bps is not None else config.default_threshold_bps_grid)
    )
    windows = args.windows if args.windows is not None else config.default_windows
    fetch_start = start_ts - (windows + 2) * interval_seconds
    candles = coinbase_candles(fetch_start, start_ts + 60, product_id=config.product_id)
    candles_by_ts = candle_maps(candles)
    backtest_rows, records_by_minute = evaluate_backtests(
        candles_by_ts,
        start_ts,
        windows,
        decision_minutes,
        threshold_grid,
        interval_minutes=config.interval_minutes,
    )
    strategy = choose_strategy(backtest_rows)
    selected_decision_minute = strategy_decision_minute(
        strategy, decision_minutes[0] if decision_minutes else 1
    )
    records = records_by_minute.get(selected_decision_minute, [])
    state = RealtimeWindowState(start_ts=start_ts, end_ts=end_ts)
    alerted = False
    last_logged_minute = None
    feishu_webhook = args.feishu_webhook or os.environ.get("FEISHU_WEBHOOK_URL")
    write_log_event(
        args.log_file,
        "monitor_started",
        market=config.key,
        asset_label=config.asset_label,
        interval_minutes=config.interval_minutes,
        product_id=config.product_id,
        polymarket_title=snapshot["title"],
        event_slug=snapshot["event_slug"],
        prices=snapshot["prices"],
        windows=windows,
        decision_minutes=decision_minutes,
        threshold_bps_grid=threshold_grid,
        selected_strategy=strategy,
        selected_decision_minute=selected_decision_minute,
        records=len(records),
        end_at=snapshot["end_at"],
    )

    subscribe = {
        "type": "subscribe",
        "product_ids": [config.product_id],
        "channels": ["ticker"],
    }
    async with websockets.connect(COINBASE_WS_URL, ping_interval=20) as websocket:
        await websocket.send(json.dumps(subscribe))
        write_log_event(args.log_file, "websocket_subscribed", product_id=config.product_id)
        while int(dt.datetime.now(dt.timezone.utc).timestamp()) < end_ts:
            message = json.loads(await websocket.recv())
            if message.get("type") != "ticker" or "price" not in message:
                continue
            timestamp = int(iso_to_dt(message["time"].replace("Z", "+00:00")).timestamp())
            state.update(timestamp, float(message["price"]))
            signal = realtime_signal_from_state(
                snapshot,
                state,
                selected_decision_minute,
                strategy,
                args.bankroll,
                asset_label=config.asset_label,
            )
            if not args.quiet:
                print(json.dumps({"market": snapshot["title"], "signal": signal}, ensure_ascii=False), flush=True)
            if should_log_tick(last_logged_minute, signal):
                write_log_event(
                    args.log_file,
                    "tick_signal",
                    market=config.key,
                    price=float(message["price"]),
                    signal=signal,
                )
                last_logged_minute = signal.get("elapsed_minutes")
            if signal.get("action") == "RESEARCH_CANDIDATE" and not alerted:
                payload = {
                    "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
                    "label": f"Polymarket {config.asset_label} {config.interval_minutes}m 信号",
                    "market": snapshot,
                    "backtest": {
                        "records": len(records),
                        "strategies": backtest_rows,
                        "selected_strategy": strategy,
                    },
                    "signal": signal,
                    "note": "Paper-only alert. This program does not place real orders.",
                }
                write_alert(args.alert_file, payload)
                run_alert_command(args.alert_command, payload)
                feishu_result = send_feishu_alert(feishu_webhook, payload)
                write_log_event(
                    args.log_file,
                    "research_candidate_alerted",
                    market=config.key,
                    signal=signal,
                    feishu_result=feishu_result,
                    alert_file=args.alert_file,
                )
                alerted = True
        write_log_event(
            args.log_file,
            "monitor_finished",
            market=config.key,
            alerted=alerted,
            polymarket_title=snapshot["title"],
        )
        return {"alerted": alerted, "market": snapshot["title"]}


async def monitor_forever(args):
    normalize_monitor_args(args)
    while True:
        try:
            result = await monitor_websocket(args)
            write_log_event(args.log_file, "loop_cycle_finished", result=result)
        except Exception as exc:
            write_log_event(
                args.log_file,
                "loop_cycle_error",
                error_type=type(exc).__name__,
                error=str(exc),
            )
        await asyncio.sleep(args.loop_sleep_seconds)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--market", choices=sorted(MARKET_CONFIGS), default="eth15m")
    parser.add_argument("--slug", default="")
    parser.add_argument("--auto-current", action="store_true")
    parser.add_argument("--windows", type=int, default=None)
    parser.add_argument("--decision-minute", type=int, default=None)
    parser.add_argument("--decision-minutes", default="")
    parser.add_argument("--threshold-bps", type=int, default=None)
    parser.add_argument("--threshold-bps-grid", default="")
    parser.add_argument("--bankroll", type=float, default=15)
    parser.add_argument("--monitor-ws", action="store_true")
    parser.add_argument("--loop", action="store_true")
    parser.add_argument("--loop-sleep-seconds", type=float, default=3.0)
    parser.add_argument("--quiet", action="store_true")
    parser.add_argument("--alert-file", default="reports/eth_15m_alerts.jsonl")
    parser.add_argument("--alert-command", default="")
    parser.add_argument("--log-file", default="")
    parser.add_argument("--feishu-webhook", default="")
    parser.add_argument("--send-feishu-test", action="store_true")
    args = parser.parse_args()

    if args.send_feishu_test:
        webhook = args.feishu_webhook or os.environ.get("FEISHU_WEBHOOK_URL")
        test_payload = {
            "market": {"title": "Feishu webhook connection test"},
            "backtest": {
                "selected_strategy": {
                    "name": "connection_test",
                    "accuracy": None,
                    "win_rate_lower_bound": None,
                    "trades": "N/A",
                }
            },
            "signal": {
                "pick": "TEST",
                "entry_price": None,
                "edge_after_margin": None,
                "reason": "If you see this message, Polymarket alerts can reach Feishu.",
                "paper_order": {
                    "side": "TEST",
                    "limit_price": None,
                    "suggested_size_usd": None,
                },
            },
        }
        print(json.dumps(send_feishu_alert(webhook, test_payload), ensure_ascii=False, indent=2))
        return

    if args.monitor_ws:
        normalize_monitor_args(args)
        if args.loop:
            asyncio.run(monitor_forever(args))
            return
        print(json.dumps(asyncio.run(monitor_websocket(args)), ensure_ascii=False, indent=2))
        return

    config = market_config(args.market)
    event = get_current_event(config) if args.auto_current or not args.slug else get_event(args.slug)
    snapshot = market_snapshot(event)
    market_end_ts = int(iso_to_dt(snapshot["end_at"]).timestamp())
    now_ts = int(dt.datetime.now(dt.timezone.utc).timestamp())
    decision_minutes = (
        parse_int_csv(args.decision_minutes)
        or ([args.decision_minute] if args.decision_minute is not None else config.default_decision_minutes)
    )
    threshold_grid = (
        parse_int_csv(args.threshold_bps_grid)
        or ([args.threshold_bps] if args.threshold_bps is not None else config.default_threshold_bps_grid)
    )
    windows = args.windows if args.windows is not None else config.default_windows
    fetch_start = min(market_end_ts, now_ts) - (windows + 2) * config.interval_minutes * 60
    fetch_end = max(market_end_ts, now_ts) + 5 * 60
    candles = coinbase_candles(fetch_start, fetch_end, product_id=config.product_id)
    candles_by_ts = candle_maps(candles)
    backtest_rows, records_by_minute = evaluate_backtests(
        candles_by_ts,
        min(market_end_ts, now_ts),
        windows,
        decision_minutes,
        threshold_grid,
        interval_minutes=config.interval_minutes,
    )
    strategy = choose_strategy(backtest_rows)
    selected_decision_minute = strategy_decision_minute(
        strategy, decision_minutes[0] if decision_minutes else 1
    )
    records = records_by_minute.get(selected_decision_minute, [])
    signal = current_signal(
        snapshot, candles_by_ts, selected_decision_minute, strategy, config=config
    )

    result = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "source_note": f"Polymarket resolves this market with Chainlink {config.asset_label}/USD. Coinbase {config.product_id} candles are used here as a liquid proxy for feature/backtest signals, so live results may differ from Chainlink.",
        "market": snapshot,
        "model": {
            "market": config.key,
            "asset_label": config.asset_label,
            "interval_minutes": config.interval_minutes,
            "product_id": config.product_id,
            "slug_prefix": config.slug_prefix,
        },
        "settings": {
            "windows": windows,
            "decision_minutes": decision_minutes,
            "threshold_bps_grid": threshold_grid,
            "selected_decision_minute": selected_decision_minute,
        },
        "backtest": {
            "records": len(records),
            "strategies": backtest_rows,
            "selected_strategy": strategy,
        },
        "signal": signal,
    }
    if signal.get("entry_price") is not None and strategy:
        result["stake"] = kelly_stake(
            args.bankroll, strategy["accuracy"], signal["entry_price"]
        )
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
