"""Tests for scoring, levels, and high score persistence."""

import json
import pytest
from pathlib import Path

from score import ScoreManager, HIGH_SCORE_FILE, LINE_SCORES


class TestScoring:
    def test_initial_state(self):
        sm = ScoreManager()
        assert sm.score == 0
        assert sm.level == 1
        assert sm.lines_cleared == 0

    def test_single_line_level_1(self):
        sm = ScoreManager()
        sm.add_lines(1)
        assert sm.score == 100

    def test_double_line_level_1(self):
        sm = ScoreManager()
        sm.add_lines(2)
        assert sm.score == 300

    def test_triple_line_level_1(self):
        sm = ScoreManager()
        sm.add_lines(3)
        assert sm.score == 500

    def test_tetris_level_1(self):
        sm = ScoreManager()
        sm.add_lines(4)
        assert sm.score == 800

    def test_score_multiplied_by_level(self):
        sm = ScoreManager()
        sm._level = 2
        sm.add_lines(1)
        assert sm.score == 200

    def test_invalid_line_count_ignored(self):
        sm = ScoreManager()
        sm.add_lines(5)
        assert sm.score == 0
        assert sm.lines_cleared == 0


class TestLevels:
    def test_level_starts_at_1(self):
        sm = ScoreManager()
        assert sm.level == 1

    def test_level_increases_every_10_lines(self):
        sm = ScoreManager()
        for _ in range(10):
            sm.add_lines(1)
        assert sm.level == 2
        for _ in range(10):
            sm.add_lines(1)
        assert sm.level == 3

    def test_drop_interval_level_1(self):
        sm = ScoreManager()
        assert sm.get_drop_interval() == pytest.approx(1.0)

    def test_higher_levels_faster(self):
        sm = ScoreManager()
        for _ in range(10):
            sm.add_lines(1)
        assert sm.get_drop_interval() < 1.0

    def test_drop_interval_never_below_minimum(self):
        sm = ScoreManager()
        sm._lines = 1000
        sm._level = 1 + sm._lines // 10
        assert sm.get_drop_interval() >= 0.1


class TestHighScore:
    def test_load_missing_file(self, tmp_path, monkeypatch):
        monkeypatch.setattr("score.HIGH_SCORE_FILE", tmp_path / "nope")
        sm = ScoreManager()
        assert sm.load_high_score() == 0

    def test_save_and_load(self, tmp_path, monkeypatch):
        f = tmp_path / "score.json"
        monkeypatch.setattr("score.HIGH_SCORE_FILE", f)
        sm = ScoreManager()
        sm.add_lines(4)
        sm.save_high_score()
        assert sm.load_high_score() == 800

    def test_only_saves_if_higher(self, tmp_path, monkeypatch):
        f = tmp_path / "score.json"
        f.write_text(json.dumps({"high_score": 9999}))
        monkeypatch.setattr("score.HIGH_SCORE_FILE", f)
        sm = ScoreManager()
        sm.add_lines(1)
        sm.save_high_score()
        assert sm.load_high_score() == 9999
