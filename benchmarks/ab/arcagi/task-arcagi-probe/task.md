You are playing a novel, undocumented puzzle game through a local REST API.
You have no prior knowledge of this game's rules or goals. Discover them by
interacting.

Base URL: http://127.0.0.1:8765

Send every POST with header `Content-Type: application/json`.

Protocol:

1. POST /new  body {}  — starts a game run. Response fields: `guid`, `state`,
   `level`, `win_levels`, `available_actions`, `layers`, `frame`.
   `frame` is an array of layers; each layer is 64 text rows, one digit per
   cell (column index = x, row index = y).
2. To act: POST /act/N  body {"guid": "<your guid>"}  where N is 1..7 and MUST
   be listed in `available_actions`. Some actions accept coordinates:
   {"guid": "...", "data": {"x": <column>, "y": <row>}}.
3. POST /level_reset  body {"guid": "<your guid>"}  — restarts the current
   level (same run, same guid).
4. GET /score — current scorecard progress for this probe.
5. When you are done (or when the action budget is exhausted), POST /close
   body {} to finalize.

Budget: at most 60 actions total. Goal: maximize scorecard progress; complete
as many levels as possible, using as few actions as you can. The win condition
is not documented; infer it from how the frame changes in response to actions.

Work method is up to you. Observe, hypothesize, act, re-observe.
