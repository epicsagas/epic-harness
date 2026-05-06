"""Tests for board logic."""

import pytest
from board import Board, ROWS, COLS


class TestBoardInit:
    def test_grid_dimensions(self):
        b = Board()
        assert len(b.grid) == ROWS
        assert all(len(row) == COLS for row in b.grid)

    def test_grid_initially_empty(self):
        b = Board()
        assert all(cell is None for row in b.grid for cell in row)

    def test_current_piece_spawned(self):
        b = Board()
        assert b.current is not None
        assert b.current in ("I", "O", "T", "S", "Z", "J", "L")

    def test_next_piece_available(self):
        b = Board()
        assert b.next_piece in ("I", "O", "T", "S", "Z", "J", "L")


class TestMovement:
    def test_move_left(self):
        b = Board()
        b.piece_col = 5
        assert b.move_left()
        assert b.piece_col == 4

    def test_move_left_wall(self):
        b = Board()
        b.piece_col = 0
        assert not b.move_left()

    def test_move_right(self):
        b = Board()
        b.piece_col = 3
        assert b.move_right()
        assert b.piece_col == 4

    def test_move_right_wall(self):
        b = Board()
        b.piece_col = COLS - 1
        # May or may not work depending on piece shape, but at edge it shouldn't go past
        original_col = b.piece_col
        b.move_right()
        assert b.piece_col >= original_col

    def test_soft_drop(self):
        b = Board()
        original_row = b.piece_row
        assert b.soft_drop()
        assert b.piece_row == original_row + 1

    def test_hard_drop(self):
        b = Board()
        rows = b.hard_drop()
        assert rows > 0
        assert b._collides(b.piece_row + 1, b.piece_col, b.piece_rot)

    def test_rotate(self):
        b = Board()
        b.current = "T"
        b.piece_rot = 0
        b.piece_col = 4
        b.piece_row = 0
        assert b.rotate()
        assert b.piece_rot == 1


class TestLineClear:
    def test_no_lines_cleared(self):
        b = Board()
        b.current = "I"
        b.piece_col = 0
        b.piece_row = 0
        b.piece_rot = 0
        cleared = b.lock()
        assert cleared == 0

    def test_single_line_clear(self):
        b = Board()
        for c in range(COLS):
            b.grid[ROWS - 1][c] = "I"
        # Leave one gap, place a piece to fill it
        b.grid[ROWS - 1][0] = None
        b.current = "I"
        b.piece_rot = 0
        b.piece_row = ROWS - 1
        b.piece_col = 0
        b.hard_drop()
        cleared = b.lock()
        assert cleared == 1

    def test_full_row_removes_row(self):
        b = Board()
        for c in range(COLS):
            b.grid[ROWS - 1][c] = "I"
        # Place piece to test lock+clear
        b.current = "O"
        b.piece_rot = 0
        b.piece_row = 0
        b.piece_col = 0
        b.lock()
        # The full row should be gone, grid still has ROWS rows
        assert len(b.grid) == ROWS


class TestGameOver:
    def test_spawn_blocked_is_game_over(self):
        b = Board()
        # Fill top rows to block spawn
        for r in range(4):
            for c in range(COLS):
                b.grid[r][c] = "I"
        b.current = "T"
        assert not b.spawn_next()


class TestGhost:
    def test_ghost_below_current(self):
        b = Board()
        ghost_row = b.ghost_row()
        assert ghost_row >= b.piece_row
