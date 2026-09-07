#!/usr/bin/env python3
"""ARC-AGI-3 probe bridge: minimal REST wrapper for a headless agent probe.

Boots the official toolkit (arc_agi) in ONLINE mode in-process, opens ONE
scorecard, and exposes a tiny JSON API on localhost so a headless agent can
play via curl without ever seeing the API key:

  GET  /healthcheck         liveness
  POST /new                 {} -> new game run (full reset, new guid)
  POST /act/N               {"guid":..., "data":{"x":..,"y":..}?} -> frame view
  POST /level_reset         {"guid":...} -> level reset (same game run)
  GET  /score               compact scorecard view
  POST /close               close scorecard, write final JSON to $RESULT_JSON

Frames render as text rows (one char per cell per layer) to bound agent token
cost. Server-side guards: $ACTION_CAP steps, $MAX_NEW game runs. The key is
read from ARC_API_KEY/ARCPRIZE_API_KEY and never logged or returned.
"""
import json
import logging
import os
import threading

from arc_agi import Arcade, OperationMode
from arcengine import GameAction
from flask import Flask, jsonify, request

logging.getLogger("arc_agi").setLevel(logging.WARNING)

GAME_ID = os.environ.get("GAME_ID", "ls20")
ACTION_CAP = int(os.environ.get("ACTION_CAP", "60"))
MAX_NEW = int(os.environ.get("MAX_NEW", "3"))
RESULT_JSON = os.environ.get("RESULT_JSON", "results/probe-latest.json")
PORT = int(os.environ.get("ARCAGI_PORT", "8765"))

lock = threading.Lock()
state = {"steps": 0, "new_calls": 0, "closed": False}
holder = {"game": None}


def render_frame(fd) -> dict:
    layers = []
    for lay in (fd.frame or [])[:3]:
        layers.append(["".join(str(int(v)) for v in row) for row in lay])
    return {
        "guid": fd.guid,
        "state": str(fd.state),
        "level": fd.levels_completed,
        "win_levels": fd.win_levels,
        "available_actions": fd.available_actions,
        "layers": len(layers),
        "frame": layers,
    }


def score_view(arc) -> dict:
    sc = arc.get_scorecard()
    out = {"card_id": sc.card_id, "score": sc.score, "runs": []}
    for env in sc.environments:
        if not env.id.startswith(GAME_ID):
            continue
        for r in env.runs:
            out["runs"].append({
                "guid": r.guid,
                "score": r.score,
                "levels_completed": r.levels_completed,
                "actions": r.actions,
                "resets": r.resets,
                "state": str(r.state),
                "level_scores": r.level_scores,
            })
    return out


def main() -> None:
    key = os.environ.get("ARC_API_KEY") or os.environ.get("ARCPRIZE_API_KEY") or ""
    if not key:
        raise SystemExit("ERR: ARC_API_KEY/ARCPRIZE_API_KEY not set")
    arc = Arcade(arc_api_key=key, operation_mode=OperationMode.ONLINE)
    env = next(x for x in arc.available_environments if x.game_id.startswith(GAME_ID))
    arc.create_scorecard(tags=["agent", "probe"])
    card_id = arc.get_scorecard().card_id
    holder["game"] = arc.make(env.game_id)
    print(f"BRIDGE READY game={env.game_id} card={card_id}", flush=True)

    app = Flask(__name__)

    @app.get("/healthcheck")
    def healthcheck():
        return "okay"

    @app.post("/new")
    def new_game():
        with lock:
            if state["closed"]:
                return jsonify({"error": "scorecard closed"}), 409
            if state["new_calls"] >= MAX_NEW:
                return jsonify({"error": f"game-run cap reached ({MAX_NEW})"}), 429
            state["new_calls"] += 1
            game = arc.make(env.game_id)
            holder["game"] = game
            fd = game.reset()  # deterministic first frame after auto-reset in make()
        return jsonify({"full_reset": True, **render_frame(fd)})

    @app.post("/act/<int:n>")
    def act(n: int):
        if not (1 <= n <= 7):
            return jsonify({"error": "action N must be 1..7"}), 400
        body = request.get_json(silent=True) or {}
        guid = body.get("guid")
        if not guid:
            return jsonify({"error": "missing guid (POST /new first)"}), 400
        with lock:
            if state["closed"]:
                return jsonify({"error": "scorecard closed"}), 409
            if state["steps"] >= ACTION_CAP:
                return jsonify({"error": f"action cap reached ({ACTION_CAP}); "
                                         "finish: GET /score then POST /close"}), 429
            state["steps"] += 1
        try:
            fd = holder["game"].step(GameAction[f"ACTION{n}"], data=body.get("data"))
        except Exception as ex:  # toolkit/API validation errors -> agent-readable
            return jsonify({"error": f"{type(ex).__name__}: {ex}"[:300]}), 200
        if fd is None:
            return jsonify({"error": "step failed (see bridge stderr)"}), 200
        return jsonify({"action_used": n, **render_frame(fd)})

    @app.post("/level_reset")
    def level_reset():
        body = request.get_json(silent=True) or {}
        guid = body.get("guid")
        if not guid:
            return jsonify({"error": "missing guid"}), 400
        with lock:
            if state["closed"]:
                return jsonify({"error": "scorecard closed"}), 409
        try:
            fd = holder["game"].reset()
        except Exception as ex:
            return jsonify({"error": f"{type(ex).__name__}: {ex}"[:300]}), 200
        return jsonify({"level_reset": True, **render_frame(fd)})

    @app.get("/score")
    def score():
        with lock:
            out = score_view(arc)
        out["probe_steps"] = state["steps"]
        return jsonify(out)

    @app.post("/close")
    def close():
        with lock:
            if state["closed"]:
                return jsonify({"error": "already closed"}), 409
            state["closed"] = True
            final = score_view(arc)
            arc.close_scorecard()
            os.makedirs(os.path.dirname(RESULT_JSON) or ".", exist_ok=True)
            with open(RESULT_JSON, "w") as f:
                json.dump({"game": env.game_id, "steps_used": state["steps"],
                           "final": final}, f, indent=2, default=str)
        return jsonify({"closed": True, "written": RESULT_JSON, **final})

    app.run(host="127.0.0.1", port=PORT, threaded=True)


if __name__ == "__main__":
    main()
