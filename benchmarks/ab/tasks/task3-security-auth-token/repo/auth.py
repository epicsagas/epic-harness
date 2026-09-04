import base64
import hashlib
import hmac
import json
import time
from typing import Any, Dict, Optional

SECRET_KEY = "super-secret-key-for-auth-bench"


def generate_token(payload: Dict[str, Any], secret: str = SECRET_KEY) -> str:
    """Generate a simple HMAC-signed token: header.payload.signature."""
    header = {"alg": "HS256", "typ": "JWT"}
    hdr_b64 = base64.urlsafe_b64encode(json.dumps(header).encode()).decode().rstrip("=")
    pay_b64 = base64.urlsafe_b64encode(json.dumps(payload).encode()).decode().rstrip("=")
    signing_input = f"{hdr_b64}.{pay_b64}".encode()
    sig = hmac.new(secret.encode(), signing_input, hashlib.sha256).digest()
    sig_b64 = base64.urlsafe_b64encode(sig).decode().rstrip("=")
    return f"{hdr_b64}.{pay_b64}.{sig_b64}"


def verify_token(token: str, secret: str = SECRET_KEY) -> Optional[Dict[str, Any]]:
    """
    Verify HMAC signature and expiration of the token.
    Returns decoded payload if valid, None otherwise.
    BUG 1: Does not properly check expiration (`exp`).
    BUG 2: Vulnerable to forged signature comparison (insecure comparison).
    BUG 3: Vulnerable to malformed token crash.
    """
    try:
        parts = token.split(".")
        if len(parts) != 3:
            return None
        hdr_b64, pay_b64, sig_b64 = parts
        signing_input = f"{hdr_b64}.{pay_b64}".encode()
        
        # Calculate expected signature
        expected_sig = hmac.new(secret.encode(), signing_input, hashlib.sha256).digest()
        expected_b64 = base64.urlsafe_b64encode(expected_sig).decode().rstrip("=")
        
        # BUG: non-constant time comparison and accepts empty/none
        if sig_b64 != expected_b64:
            return None
            
        # Decode payload
        padding = "=" * ((4 - len(pay_b64) % 4) % 4)
        pay_json = base64.urlsafe_b64decode(pay_b64 + padding).decode()
        payload = json.loads(pay_json)
        
        # BUG: Missing expiration check!
        # If payload has 'exp' and exp < current time, should return None
        return payload
    except Exception:
        return None


def get_user_session(db_cursor: Any, user_id: str) -> Optional[Dict[str, Any]]:
    """
    Query database cursor for user session.
    BUG: Formats string directly into query instead of parameterized query.
    """
    # BUG: SQL Injection vulnerability + breaks when db_cursor expects parameters
    query = f"SELECT id, username, role FROM users WHERE id = '{user_id}'"
    db_cursor.execute(query)
    row = db_cursor.fetchone()
    if not row:
        return None
    return {"id": row[0], "username": row[1], "role": row[2]}
