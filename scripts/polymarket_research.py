#!/usr/bin/env python3
import argparse
import datetime as dt
import json
import re
import statistics
import time
import urllib.parse
import urllib.request


UA = {"User-Agent": "curl/8.7.1", "Accept": "application/json"}
CRYPTO_TAG_ID = 21
ASSET_RE = re.compile(
    r"bitcoin|btc|ethereum|eth\b|solana|xrp|microstrategy|dogecoin|bnb", re.I
)


def fetch_json(base, params=None, timeout=12):
    url = base + (("?" + urllib.parse.urlencode(params)) if params else "")
    req = urllib.request.Request(url, headers=UA)
    with urllib.request.urlopen(req, timeout=timeout) as response:
        return json.load(response)


def parse_array(value):
    if isinstance(value, list):
        return value
    if not value:
        return []
    try:
        return json.loads(value)
    except json.JSONDecodeError:
        return []


def iso_to_ts(value):
    return int(dt.datetime.fromisoformat(value.replace("Z", "+00:00")).timestamp())


def get_spot_prices():
    return fetch_json(
        "https://api.coingecko.com/api/v3/simple/price",
        {
            "ids": "bitcoin,ethereum,solana,ripple,dogecoin,binancecoin",
            "vs_currencies": "usd",
            "include_24hr_change": "true",
        },
    )


def get_active_markets(limit):
    markets = fetch_json(
        "https://gamma-api.polymarket.com/markets",
        {
            "limit": limit,
            "active": "true",
            "closed": "false",
            "tag_id": CRYPTO_TAG_ID,
            "order": "volume24hr",
            "ascending": "false",
        },
    )
    rows = []
    for market in markets:
        question = market.get("question") or ""
        if not ASSET_RE.search(question):
            continue
        prices = parse_array(market.get("outcomePrices"))
        if len(prices) != 2:
            continue
        try:
            yes = float(prices[0])
        except (TypeError, ValueError):
            continue
        rows.append(
            {
                "question": question,
                "slug": market.get("slug"),
                "endDate": market.get("endDate"),
                "yes": yes,
                "no": round(1 - yes, 4),
                "spread": market.get("spread"),
                "bestBid": market.get("bestBid"),
                "bestAsk": market.get("bestAsk"),
                "volume24hr": market.get("volume24hr"),
                "liquidityNum": market.get("liquidityNum"),
            }
        )
    return rows


def price_at(token_id, center_ts):
    try:
        history = fetch_json(
            "https://clob.polymarket.com/prices-history",
            {
                "market": token_id,
                "startTs": center_ts - 1800,
                "endTs": center_ts + 1800,
                "interval": "1h",
                "fidelity": 60,
            },
            timeout=7,
        ).get("history", [])
    except Exception:
        return None
    if not history:
        return None
    nearest = min(history, key=lambda point: abs(float(point.get("t", 0)) - center_ts))
    try:
        return float(nearest["p"])
    except (KeyError, TypeError, ValueError):
        return None


def get_closed_sample(limit):
    closed = []
    for offset in range(0, max(limit * 5, 100), 100):
        closed.extend(
            fetch_json(
                "https://gamma-api.polymarket.com/markets",
                {
                    "limit": 100,
                    "offset": offset,
                    "closed": "true",
                    "active": "true",
                    "tag_id": CRYPTO_TAG_ID,
                    "order": "updatedAt",
                    "ascending": "false",
                },
            )
        )
        time.sleep(0.05)

    rows = []
    seen = set()
    for market in closed:
        market_id = market.get("id")
        if market_id in seen:
            continue
        seen.add(market_id)
        question = market.get("question") or ""
        if not ASSET_RE.search(question):
            continue
        outcomes = parse_array(market.get("outcomes"))
        prices = parse_array(market.get("outcomePrices"))
        tokens = parse_array(market.get("clobTokenIds"))
        if len(outcomes) != 2 or len(prices) != 2 or len(tokens) != 2:
            continue
        try:
            final_yes = float(prices[0])
            end_ts = iso_to_ts(market["endDate"])
        except (KeyError, TypeError, ValueError):
            continue
        if final_yes not in (0.0, 1.0):
            continue
        rows.append((market, end_ts, tokens, final_yes))

    return sorted(rows, key=lambda row: float(row[0].get("volumeNum") or 0), reverse=True)[
        :limit
    ]


def evaluate_directional(records, key):
    trades = []
    for record in records:
        price = record.get(key)
        if price is None or not 0 < price < 1 or abs(price - 0.5) < 1e-9:
            continue
        side = "YES" if price > 0.5 else "NO"
        entry = price if side == "YES" else 1 - price
        won = (side == "YES" and record["final_yes"] == 1) or (
            side == "NO" and record["final_yes"] == 0
        )
        profit = (1 if won else 0) - entry
        trades.append({"won": won, "profit": profit, "roi": profit / entry, "p": price})
    if not trades:
        return {"n": 0}
    wins = sum(1 for trade in trades if trade["won"])
    return {
        "n": len(trades),
        "wins": wins,
        "hit": round(wins / len(trades), 4),
        "profit_per_share": round(sum(trade["profit"] for trade in trades), 4),
        "avg_roi": round(statistics.mean(trade["roi"] for trade in trades), 4),
        "median_p": round(statistics.median(trade["p"] for trade in trades), 4),
    }


def backtest(limit):
    records = []
    for market, end_ts, tokens, final_yes in get_closed_sample(limit):
        records.append(
            {
                "question": market.get("question"),
                "final_yes": final_yes,
                "p24": price_at(tokens[0], end_ts - 24 * 3600),
                "p6": price_at(tokens[0], end_ts - 6 * 3600),
            }
        )
        time.sleep(0.05)
    return {
        "sample_markets": len(records),
        "p24_success": sum(record["p24"] is not None for record in records),
        "p6_success": sum(record["p6"] is not None for record in records),
        "directional_24h": evaluate_directional(records, "p24"),
        "directional_6h": evaluate_directional(records, "p6"),
    }


def build_signals(active_markets, spot):
    btc = spot.get("bitcoin", {}).get("usd")
    signals = []
    for market in active_markets:
        q = market["question"].lower()
        yes = market["yes"]
        if "bitcoin dip to $75,000 in may" in q:
            reason = "BTC is near 77.5k, so a touch of 75k needs roughly a 3% drawdown before June 1."
            stance = "WATCH YES" if btc and btc < 78000 and yes <= 0.58 else "WAIT"
            signals.append({**market, "stance": stance, "reason": reason})
        elif "bitcoin reach $85,000 in may" in q:
            reason = "Needs about a 10% move from spot in roughly 11 days; recent ETF flow backdrop is not supportive."
            stance = "AVOID YES / WATCH NO" if btc and btc < 80000 and yes >= 0.09 else "WAIT"
            signals.append({**market, "stance": stance, "reason": reason})
        elif "bitcoin reach $90,000 in may" in q:
            reason = "Requires an even larger short-window rally; current price and flow backdrop make YES unattractive."
            stance = "AVOID YES"
            signals.append({**market, "stance": stance, "reason": reason})
        elif "microstrategy sells any bitcoin" in q:
            reason = "Company-action market; no clear data edge without filings/company-specific monitoring."
            signals.append({**market, "stance": "NO EDGE", "reason": reason})
    return signals


def model_gate(backtest_result):
    six_hour = backtest_result.get("directional_6h", {})
    twenty_four_hour = backtest_result.get("directional_24h", {})
    reasons = []
    if six_hour.get("n", 0) < 20:
        reasons.append("6h backtest has fewer than 20 trades")
    if six_hour.get("avg_roi", -1) <= 0:
        reasons.append("6h directional rule is not profitable")
    if twenty_four_hour.get("n", 0) < 10:
        reasons.append("24h backtest has fewer than 10 trades")
    if twenty_four_hour.get("avg_roi", -1) <= 0:
        reasons.append("24h directional rule is not profitable")
    if reasons:
        return {
            "decision": "NO_REAL_TRADE",
            "confidence": "low",
            "reasons": reasons,
        }
    return {
        "decision": "RESEARCH_SIGNALS_ONLY",
        "confidence": "medium",
        "reasons": ["historical directional rule passed minimum sample and ROI gates"],
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--active-limit", type=int, default=100)
    parser.add_argument("--backtest-limit", type=int, default=20)
    args = parser.parse_args()

    active = get_active_markets(args.active_limit)
    spot = get_spot_prices()
    bt = backtest(args.backtest_limit)
    result = {
        "generated_at_utc": dt.datetime.now(dt.timezone.utc).isoformat(),
        "spot": spot,
        "backtest": bt,
        "model_gate": model_gate(bt),
        "top_active": active[:20],
    }
    result["signals"] = build_signals(active, spot)
    print(json.dumps(result, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
