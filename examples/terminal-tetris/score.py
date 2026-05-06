"""Scoring, level progression, and high score persistence for Terminal Tetris."""

import json
from pathlib import Path

HIGH_SCORE_FILE = Path.home() / ".terminal-tetris-score"

LINE_SCORES = {1: 100, 2: 300, 3: 500, 4: 800}
LINES_PER_LEVEL = 10
BASE_INTERVAL = 1.0
INTERVAL_DECREASE = 0.08
MIN_INTERVAL = 0.1


class ScoreManager:
    def __init__(self):
        self._score = 0
        self._level = 1
        self._lines = 0

    @property
    def score(self) -> int:
        return self._score

    @property
    def level(self) -> int:
        return self._level

    @property
    def lines_cleared(self) -> int:
        return self._lines

    def add_lines(self, count: int):
        if count not in LINE_SCORES:
            return
        self._score += LINE_SCORES[count] * self._level
        self._lines += count
        self._level = 1 + self._lines // LINES_PER_LEVEL

    def get_drop_interval(self) -> float:
        interval = BASE_INTERVAL - (self._level - 1) * INTERVAL_DECREASE
        return max(interval, MIN_INTERVAL)

    def load_high_score(self) -> int:
        try:
            data = json.loads(HIGH_SCORE_FILE.read_text())
            return data.get("high_score", 0)
        except (OSError, json.JSONDecodeError, KeyError):
            return 0

    def save_high_score(self):
        current_high = self.load_high_score()
        if self._score > current_high:
            HIGH_SCORE_FILE.write_text(json.dumps({"high_score": self._score}))
