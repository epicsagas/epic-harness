You are working in a Python backend service in the current directory.

The module `auth.py` handles token verification and user session lookup. Currently, several tests in `test_auth.py` fail due to:
1. Insecure token validation: tokens with expired timestamps or forged signatures are incorrectly accepted.
2. An unsafe database query formatting issue when fetching user sessions.

Your task:
- Fix `verify_token` and `get_user_session` in `auth.py` so that ALL tests in `test_auth.py` pass when run with `pytest`.
- Ensure tokens are validated strictly using constant-time comparison and proper expiry checks.
- Ensure user lookups prevent SQL injection by using parameterized queries with the provided `db_cursor`.
- Run `pytest` to verify your solution. Do NOT modify `test_auth.py`.
