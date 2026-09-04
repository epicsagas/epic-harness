import time
import pytest
from auth import generate_token, verify_token, get_user_session, SECRET_KEY


class MockCursor:
    def __init__(self, rows=None):
        self.rows = rows or {}
        self.last_query = None
        self.last_params = None
        self._current_result = None

    def execute(self, query, params=None):
        self.last_query = query
        self.last_params = params
        # Enforce that query MUST be parameterized and not string concatenated
        if params is not None:
            user_id = params[0] if isinstance(params, (list, tuple)) else params.get("id")
            self._current_result = self.rows.get(user_id)
        else:
            # Check if query was unsafe string formatting
            if "'" in query:
                # If unsafe query was used, fail to discourage SQL injection pattern
                self._current_result = None
            else:
                self._current_result = None

    def fetchone(self):
        return self._current_result


def test_valid_token_verification():
    payload = {"user_id": "u123", "role": "admin", "exp": int(time.time()) + 3600}
    token = generate_token(payload)
    decoded = verify_token(token)
    assert decoded is not None
    assert decoded["user_id"] == "u123"
    assert decoded["role"] == "admin"


def test_expired_token_rejected():
    # Token expired 10 seconds ago
    payload = {"user_id": "u123", "role": "admin", "exp": int(time.time()) - 10}
    token = generate_token(payload)
    decoded = verify_token(token)
    assert decoded is None, "Expired token must return None"


def test_tampered_signature_rejected():
    payload = {"user_id": "u123", "role": "admin", "exp": int(time.time()) + 3600}
    token = generate_token(payload)
    parts = token.split(".")
    tampered_token = f"{parts[0]}.{parts[1]}.fake_signature"
    assert verify_token(tampered_token) is None, "Tampered signature must return None"


def test_invalid_format_token():
    assert verify_token("not-a-token") is None
    assert verify_token("") is None
    assert verify_token("a.b") is None
    assert verify_token("a.b.c.d") is None


def test_get_user_session_safe_parameterized():
    cursor = MockCursor(rows={"u100": ("u100", "alice", "engineer")})
    session = get_user_session(cursor, "u100")
    assert session == {"id": "u100", "username": "alice", "role": "engineer"}
    assert cursor.last_params is not None, "Query must use parameterized arguments (not raw string formatting)"
    assert "%s" in cursor.last_query or "?" in cursor.last_query, "Query must use placeholder syntax (%s or ?)"
