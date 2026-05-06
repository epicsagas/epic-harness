"""Tests for tetromino piece definitions."""

from pieces import PIECES, PIECE_COLORS, PIECE_NAMES


class TestPieceDefinitions:
    def test_all_seven_pieces_exist(self):
        assert set(PIECE_NAMES) == {"I", "O", "T", "S", "Z", "J", "L"}

    def test_each_piece_has_four_rotations(self):
        for name in PIECE_NAMES:
            assert len(PIECES[name]) == 4, f"{name} should have 4 rotations"

    def test_each_rotation_has_four_cells(self):
        for name in PIECE_NAMES:
            for i, rot in enumerate(PIECES[name]):
                assert len(rot) == 4, f"{name} rotation {i} should have 4 cells"

    def test_rotation_states_are_tuples(self):
        for name in PIECE_NAMES:
            for i, rot in enumerate(PIECES[name]):
                for cell in rot:
                    assert isinstance(cell, tuple) and len(cell) == 2

    def test_i_piece_horizontal_in_spawn(self):
        cells = PIECES["I"][0]
        rows = {r for r, c in cells}
        assert len(rows) == 1, "I-piece spawn should be horizontal"

    def test_i_piece_vertical_after_rotate(self):
        cells = PIECES["I"][1]
        cols = {c for r, c in cells}
        assert len(cols) == 1, "I-piece rotation 1 should be vertical"

    def test_o_piece_identical_all_rotations(self):
        for rot in PIECES["O"]:
            assert rot == PIECES["O"][0]

    def test_piece_colors_valid(self):
        for name in PIECE_NAMES:
            assert name in PIECE_COLORS
            assert 1 <= PIECE_COLORS[name] <= 7
