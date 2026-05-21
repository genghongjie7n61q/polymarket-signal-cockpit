import pathlib
import sys
import tempfile
import types
import unittest
from unittest import mock


ROOT = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import eth_15m_platform as eth


class Eth15mPlatformTest(unittest.TestCase):
    def test_fractional_kelly_stake_is_capped_for_small_bankroll(self):
        result = eth.kelly_stake(bankroll=15, probability=0.8276, price=0.455)

        self.assertAlmostEqual(result["full_kelly_fraction"], 0.6837, places=3)
        self.assertAlmostEqual(result["suggested_fraction"], 0.0684, places=3)
        self.assertAlmostEqual(result["suggested_stake"], 1.03, places=2)

    def test_realtime_window_state_tracks_open_and_latest_return(self):
        state = eth.RealtimeWindowState(start_ts=1_000, end_ts=1_900)
        state.update(999, 100)
        state.update(1_000, 100)
        state.update(1_120, 100.8)

        self.assertEqual(state.open_price, 100)
        self.assertEqual(state.latest_price, 100.8)
        self.assertAlmostEqual(state.decision_return, 0.008)

    def test_signal_requires_model_edge_after_margin(self):
        signal = eth.policy_signal(
            pick="Up",
            entry_price=0.62,
            fair_price_after_margin=0.64,
            spread=0.01,
            min_edge=0.03,
        )

        self.assertEqual(signal["action"], "NO_TRADE")
        self.assertAlmostEqual(signal["edge_after_margin"], 0.02)

    def test_signal_allows_research_candidate_when_edge_is_large(self):
        signal = eth.policy_signal(
            pick="Up",
            entry_price=0.45,
            fair_price_after_margin=0.70,
            spread=0.01,
            min_edge=0.03,
        )

        self.assertEqual(signal["action"], "RESEARCH_CANDIDATE")
        self.assertAlmostEqual(signal["edge_after_margin"], 0.25)

    def test_price_at_or_before_uses_latest_available_candle(self):
        candles = {
            100: {"close": 1.0},
            160: {"close": 1.1},
            220: {"close": 1.2},
        }

        self.assertEqual(eth.price_at_or_before(candles, 219, "close"), 1.1)

    def test_threshold_strategy_waits_when_live_move_is_inside_threshold(self):
        snapshot = {"prices": {"Up": 0.51, "Down": 0.49}, "spread": 0.01}
        state = eth.RealtimeWindowState(start_ts=1_000, end_ts=1_900)
        state.update(1_000, 100.0)
        state.update(1_420, 100.0)
        strategy = {
            "name": "early_direction_threshold_8bps",
            "accuracy": 0.80,
            "fair_price_with_3pct_margin": 0.77,
        }

        signal = eth.realtime_signal_from_state(
            snapshot, state, decision_minute=5, strategy=strategy, bankroll=15
        )

        self.assertEqual(signal["action"], "NO_TRADE")
        self.assertIsNone(signal["pick"])
        self.assertIn("threshold", signal["reason"])

    def test_threshold_strategy_name_can_include_decision_minute(self):
        strategy = {"name": "early_direction_threshold_m6_10bps"}

        pick, reason = eth.pick_from_strategy(strategy, decision_return=0.0012)

        self.assertEqual(pick, "Up")
        self.assertIn("10bps", reason)

    def test_parse_int_csv_accepts_comma_separated_grid(self):
        self.assertEqual(eth.parse_int_csv("3,5, 7"), [3, 5, 7])

    def test_btc5m_market_config_has_independent_defaults(self):
        config = eth.market_config("btc5m")

        self.assertEqual(config.product_id, "BTC-USD")
        self.assertEqual(config.slug_prefix, "btc-updown-5m")
        self.assertEqual(config.interval_minutes, 5)
        self.assertEqual(config.default_decision_minutes, [1, 2, 3])
        self.assertEqual(config.default_threshold_bps_grid, [2, 4, 6, 8, 10])

    def test_floor_to_interval_supports_five_minute_windows(self):
        self.assertEqual(eth.floor_to_interval(1_779_326_299, 5), 1_779_326_100)

    def test_build_event_slug_uses_market_config_prefix_and_interval_floor(self):
        config = eth.market_config("btc5m")

        self.assertEqual(
            eth.build_event_slug(config, 1_779_326_299),
            "btc-updown-5m-1779326100",
        )

    def test_build_window_records_supports_five_minute_interval(self):
        candles = {}
        for ts, price in [
            (600, 99.5),
            (900, 100.0),
            (960, 100.2),
            (1020, 100.4),
            (1080, 100.6),
            (1140, 101.0),
            (1200, 101.0),
            (1260, 100.8),
            (1320, 100.7),
            (1380, 100.6),
            (1440, 100.0),
        ]:
            candles[ts] = {"open": price, "close": price}

        records = eth.build_window_records(
            candles, end_ts=1800, windows=2, decision_minute=2, interval_minutes=5
        )

        self.assertEqual(len(records), 2)
        self.assertEqual(records[0]["start_ts"], 900)
        self.assertEqual(records[0]["actual"], "Up")
        self.assertEqual(records[1]["start_ts"], 1200)
        self.assertEqual(records[1]["actual"], "Down")

    def test_choose_strategy_prefers_more_robust_confidence_bound(self):
        rows = [
            {"name": "fragile", "trades": 20, "coverage": 0.5, "wins": 17, "accuracy": 0.85},
            {"name": "robust", "trades": 200, "coverage": 0.8, "wins": 150, "accuracy": 0.75},
        ]

        selected = eth.choose_strategy(rows)

        self.assertEqual(selected["name"], "robust")
        self.assertGreater(selected["win_rate_lower_bound"], 0.68)

    def test_feishu_payload_prioritizes_action_fields(self):
        payload = {
            "market": {"title": "Ethereum Up or Down", "prices": {"Up": 0.45, "Down": 0.55}},
            "backtest": {
                "selected_strategy": {
                    "name": "early_direction_threshold_m7_12bps",
                    "accuracy": 0.9818,
                    "win_rate_lower_bound": 0.9039,
                    "trades": 55,
                }
            },
            "signal": {
                "pick": "Down",
                "entry_price": 0.55,
                "edge_after_margin": 0.12,
                "reason": "WebSocket minute 7; ETH is -0.130% from interval open.",
                "paper_order": {
                    "side": "Down",
                    "limit_price": 0.55,
                    "suggested_size_usd": 0.82,
                },
            },
        }

        message = eth.build_feishu_payload(payload)
        element = message["card"]["elements"][0]
        content = element["text"]["content"]

        self.assertEqual(message["msg_type"], "interactive")
        self.assertEqual(element["tag"], "div")
        self.assertEqual(element["text"]["tag"], "lark_md")
        self.assertIn("方向: Down", content)
        self.assertIn("限价: 0.55", content)
        self.assertIn("建议金额: $0.82", content)
        self.assertLess(content.index("方向"), content.index("策略"))

    def test_send_feishu_alert_posts_json_payload(self):
        sent = {}

        class Response:
            def __enter__(self):
                return self

            def __exit__(self, exc_type, exc, tb):
                return False

            def read(self):
                return b'{"StatusCode":0}'

        def fake_urlopen(request, timeout):
            sent["url"] = request.full_url
            sent["data"] = request.data
            sent["timeout"] = timeout
            return Response()

        with mock.patch.object(eth.urllib.request, "urlopen", side_effect=fake_urlopen):
            result = eth.send_feishu_alert("https://example.com/hook", {"signal": {}})

        self.assertEqual(result["StatusCode"], 0)
        self.assertEqual(sent["url"], "https://example.com/hook")
        self.assertIn(b'"msg_type": "interactive"', sent["data"])

    def test_write_log_event_appends_json_line(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = pathlib.Path(tmp) / "runtime.jsonl"

            eth.write_log_event(path, "monitor_started", market="btc5m", windows=576)

            row = eth.json.loads(path.read_text().strip())
            self.assertEqual(row["event"], "monitor_started")
            self.assertEqual(row["market"], "btc5m")
            self.assertEqual(row["windows"], 576)
            self.assertIn("ts_utc", row)

    def test_should_log_tick_throttles_by_minute_or_action(self):
        self.assertTrue(eth.should_log_tick(None, {"elapsed_minutes": 2, "action": "NO_TRADE"}))
        self.assertFalse(eth.should_log_tick(2, {"elapsed_minutes": 2, "action": "NO_TRADE"}))
        self.assertTrue(eth.should_log_tick(2, {"elapsed_minutes": 2, "action": "RESEARCH_CANDIDATE"}))
        self.assertTrue(eth.should_log_tick(2, {"elapsed_minutes": 3, "action": "NO_TRADE"}))

    def test_normalize_monitor_args_forces_auto_current_for_loop(self):
        args = types.SimpleNamespace(loop=True, auto_current=False, slug="fixed-slug")

        eth.normalize_monitor_args(args)

        self.assertTrue(args.auto_current)
        self.assertEqual(args.slug, "")


if __name__ == "__main__":
    unittest.main()
