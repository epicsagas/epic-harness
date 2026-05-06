"""Board state, collision detection, piece placement, and line clearing."""

import random
from pieces import PIECES, PIECE_NAMES

ROWS = 20
COLS = 10


class Board:
    def __init__(self):
        self.grid = [[None] * COLS for _ in range(ROWS)]
        self.bag = []
        self._next = self._draw()
        self.current = None
        self.piece_row = 0
        self.piece_col = 0
        self.piece_rot = 0
        self._spawn()

    def _draw(self) -> str:
        if not self.bag:
            self.bag = PIECE_NAMES[:]
            random.shuffle(self.bag)
        return self.bag.pop()

    @property
    def next_piece(self) -> str:
        return self._next

    def _spawn(self) -> bool:
        name = self._next
        self._next = self._draw()
        self.current = name
        self.piece_rot = 0
        cells = PIECES[name][0]
        min_col = min(c for r, c in cells)
        max_col = max(c for r, c in cells)
        self.piece_col = (COLS - (max_col - min_col + 1)) // 2
        self.piece_row = 0
        if self._collides(self.piece_row, self.piece_col, self.piece_rot):
            return False
        return True

    def _cells(self, row, col, rot):
        return [(row + r, col + c) for r, c in PIECES[self.current][rot]]

    def _collides(self, row, col, rot) -> bool:
        for r, c in self._cells(row, col, rot):
            if r < 0 or r >= ROWS or c < 0 or c >= COLS:
                return True
            if self.grid[r][c] is not None:
                return True
        return False

    def move_left(self) -> bool:
        if not self._collides(self.piece_row, self.piece_col - 1, self.piece_rot):
            self.piece_col -= 1
            return True
        return False

    def move_right(self) -> bool:
        if not self._collides(self.piece_row, self.piece_col + 1, self.piece_rot):
            self.piece_col += 1
            return True
        return False

    def rotate(self) -> bool:
        new_rot = (self.piece_rot + 1) % 4
        if not self._collides(self.piece_row, self.piece_col, new_rot):
            self.piece_rot = new_rot
            return True
        return False

    def soft_drop(self) -> bool:
        if not self._collides(self.piece_row + 1, self.piece_col, self.piece_rot):
            self.piece_row += 1
            return True
        return False

    def hard_drop(self) -> int:
        rows_fallen = 0
        while not self._collides(self.piece_row + 1, self.piece_col, self.piece_rot):
            self.piece_row += 1
            rows_fallen += 1
        return rows_fallen

    def ghost_row(self) -> int:
        row = self.piece_row
        while not self._collides(row + 1, self.piece_col, self.piece_rot):
            row += 1
        return row

    def lock(self) -> int:
        for r, c in self._cells(self.piece_row, self.piece_col, self.piece_rot):
            if 0 <= r < ROWS and 0 <= c < COLS:
                self.grid[r][c] = self.current
        cleared = self._clear_lines()
        return cleared

    def _clear_lines(self) -> int:
        full_rows = [r for r in range(ROWS) if all(self.grid[r][c] is not None for c in range(COLS))]
        if not full_rows:
            return 0
        for r in sorted(full_rows, reverse=True):
            del self.grid[r]
        for _ in full_rows:
            self.grid.insert(0, [None] * COLS)
        return len(full_rows)

    def spawn_next(self) -> bool:
        return self._spawn()

    def get_current_cells(self):
        return self._cells(self.piece_row, self.piece_col, self.piece_rot)

    def get_ghost_cells(self):
        return self._cells(self.ghost_row(), self.piece_col, self.piece_rot)
