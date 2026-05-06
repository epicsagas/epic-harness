"""Terminal Tetris — curses-based TUI Tetris game."""

import curses
import time

from board import Board, ROWS, COLS
from pieces import PIECE_COLORS, PIECES, init_colors
from score import ScoreManager

BLOCK = "[]"
EMPTY = "  "
GHOST_CHAR = ".."
SIDEBAR_WIDTH = 14
BOARD_TOP = 1
BOARD_LEFT = 1


def _color_of(piece_name):
    return curses.color_pair(PIECE_COLORS[piece_name])


class TetrisGame:
    def __init__(self, stdscr):
        self.stdscr = stdscr
        self.board = Board()
        self.score = ScoreManager()
        self.paused = False
        self.game_over = False
        self.last_drop = time.time()
        self.flash_rows = []
        self.flash_until = 0.0
        self._high_score = self.score.load_high_score()

        stdscr.nodelay(True)
        stdscr.timeout(50)
        curses.curs_set(0)
        init_colors()

    def run(self):
        while not self.game_over:
            self._handle_input()
            if not self.paused:
                self._tick()
            self._draw()
        self._draw_game_over()
        self.score.save_high_score()
        self._high_score = max(self._high_score, self.score.score)

    def _handle_input(self):
        key = self.stdscr.getch()
        if key == -1:
            return
        if key == ord("q") or key == ord("Q"):
            self.game_over = True
            return
        if key == ord("p") or key == ord("P"):
            self.paused = not self.paused
            return
        if self.paused:
            return
        if key == curses.KEY_LEFT:
            self.board.move_left()
        elif key == curses.KEY_RIGHT:
            self.board.move_right()
        elif key == curses.KEY_UP:
            self.board.rotate()
        elif key == curses.KEY_DOWN:
            self.board.soft_drop()
        elif key == ord(" "):
            self.board.hard_drop()
            self._lock_piece()

    def _tick(self):
        now = time.time()
        if self.flash_rows and now >= self.flash_until:
            self.flash_rows = []
        if self.flash_rows:
            return
        interval = self.score.get_drop_interval()
        if now - self.last_drop >= interval:
            if not self.board.soft_drop():
                self._lock_piece()
            self.last_drop = now

    def _lock_piece(self):
        cleared = self.board.lock()
        if cleared > 0:
            self.score.add_lines(cleared)
            self.flash_rows = [ROWS - 1 - i for i in range(cleared)]
            self.flash_until = time.time() + 0.15
        if not self.board.spawn_next():
            self.game_over = True

    def _draw(self):
        self.stdscr.erase()

        board_x = BOARD_LEFT
        board_y = BOARD_TOP
        sidebar_x = board_x + COLS * 2 + 3

        # Board border
        top_border = "+" + "--" * COLS + "+"
        self.stdscr.addstr(board_y - 1, board_x - 1, top_border)
        self.stdscr.addstr(board_y + ROWS, board_x - 1, top_border)
        for r in range(ROWS):
            self.stdscr.addstr(board_y + r, board_x - 1, "|")
            self.stdscr.addstr(board_y + r, board_x + COLS * 2, "|")

        # Ghost piece
        for r, c in self.board.get_ghost_cells():
            if 0 <= r < ROWS and 0 <= c < COLS:
                self.stdscr.addstr(board_y + r, board_x + c * 2, GHOST_CHAR)

        # Locked cells
        for r in range(ROWS):
            for c in range(COLS):
                cell = self.board.grid[r][c]
                if cell is not None:
                    if r in self.flash_rows:
                        self.stdscr.addstr(
                            board_y + r, board_x + c * 2, BLOCK, curses.A_REVERSE
                        )
                    else:
                        self.stdscr.addstr(
                            board_y + r, board_x + c * 2, BLOCK, _color_of(cell)
                        )

        # Current piece
        for r, c in self.board.get_current_cells():
            if 0 <= r < ROWS and 0 <= c < COLS:
                self.stdscr.addstr(
                    board_y + r, board_x + c * 2, BLOCK, _color_of(self.board.current)
                )

        # Sidebar
        self._draw_sidebar(sidebar_x, board_y)
        self.stdscr.refresh()

    def _draw_sidebar(self, x, y):
        self.stdscr.addstr(y, x, f"Score: {self.score.score}")
        self.stdscr.addstr(y + 1, x, f"Level: {self.score.level}")
        self.stdscr.addstr(y + 2, x, f"Lines: {self.score.lines_cleared}")
        self.stdscr.addstr(y + 3, x, f"Hi: {self._high_score}")

        # Next piece preview
        self.stdscr.addstr(y + 5, x, "Next:")
        next_name = self.board.next_piece
        cells = PIECES[next_name][0]
        for r, c in cells:
            self.stdscr.addstr(y + 7 + r, x + c * 2, BLOCK, _color_of(next_name))

        if self.paused:
            self.stdscr.addstr(y + 12, x, "PAUSED")

        # Controls
        self.stdscr.addstr(y + 14, x, "Controls:")
        self.stdscr.addstr(y + 15, x, "Arrow keys")
        self.stdscr.addstr(y + 16, x, "Space: drop")
        self.stdscr.addstr(y + 17, x, "P: pause")
        self.stdscr.addstr(y + 18, x, "Q: quit")

    def _draw_game_over(self):
        h, w = self.stdscr.getmaxyx()
        msg = "GAME OVER"
        msg2 = f"Score: {self.score.score}"
        msg3 = "Press any key to exit"
        cy = h // 2
        self.stdscr.addstr(cy - 1, (w - len(msg)) // 2, msg, curses.A_BOLD)
        self.stdscr.addstr(cy, (w - len(msg2)) // 2, msg2)
        self.stdscr.addstr(cy + 1, (w - len(msg3)) // 2, msg3)
        self.stdscr.refresh()
        self.stdscr.nodelay(False)
        self.stdscr.getch()


def main(stdscr):
    game = TetrisGame(stdscr)
    game.run()


if __name__ == "__main__":
    curses.wrapper(main)
