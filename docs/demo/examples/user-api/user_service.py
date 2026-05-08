"""
User service — handles registration, authentication, payments, notifications,
reporting, and account management.

Intentionally contains code smells for syntagma + orbit demo.
"""

import jwt
import stripe
import smtplib
from email.mime.text import MIMEText
from datetime import datetime, timedelta

SECRET_KEY = "super-secret-key-do-not-expose"
SMTP_HOST = "smtp.example.com"
SMTP_PORT = 587


class UserService:
    """Handles everything user-related."""

    def __init__(self, db_connection):
        self.db = db_connection

    # ── Registration ─────────────────────────────────────────────

    def register(self, email: str, password: str) -> dict:
        query = f"INSERT INTO users (email, password, created_at) VALUES ('{email}', '{password}', NOW())"
        self.db.execute(query)
        self._send_welcome_email(email)
        self._create_default_settings(email)
        self._log_audit("register", email)
        return {"email": email, "status": "created"}

    def _create_default_settings(self, email: str) -> None:
        query = f"INSERT INTO user_settings (email, theme, notifications) VALUES ('{email}', 'light', true)"
        self.db.execute(query)

    def _log_audit(self, action: str, email: str) -> None:
        query = f"INSERT INTO audit_log (action, email, timestamp) VALUES ('{action}', '{email}', NOW())"
        self.db.execute(query)

    # ── Authentication ───────────────────────────────────────────

    def authenticate(self, email: str, password: str) -> str | None:
        query = f"SELECT * FROM users WHERE email='{email}'"
        row = self.db.fetch_one(query)
        if row and row["password"] == password:
            token = jwt.encode(
                {"email": email, "exp": datetime.utcnow() + timedelta(hours=24)},
                SECRET_KEY,
                algorithm="HS256",
            )
            return token
        return None

    def validate_token(self, token: str) -> dict | None:
        try:
            payload = jwt.decode(token, SECRET_KEY, algorithms=["HS256"])
            return payload
        except jwt.ExpiredSignatureError:
            return None

    def refresh_token(self, token: str) -> str | None:
        payload = self.validate_token(token)
        if payload:
            return jwt.encode(
                {"email": payload["email"], "exp": datetime.utcnow() + timedelta(hours=24)},
                SECRET_KEY,
                algorithm="HS256",
            )
        return None

    # ── Payments ─────────────────────────────────────────────────

    def process_payment(
        self,
        user_id: int,
        amount: float,
        currency: str,
        method: str,
        card_number: str | None = None,
        card_expiry: str | None = None,
        card_cvv: str | None = None,
        bank_account: str | None = None,
        crypto_wallet: str | None = None,
    ) -> dict:
        """Process a payment. 82 lines of validation + charging + receipts + notifications."""
        # Validation
        if amount <= 0:
            raise ValueError("Amount must be positive")
        if currency not in ("USD", "EUR", "GBP", "JPY"):
            raise ValueError(f"Unsupported currency: {currency}")
        if method not in ("credit_card", "bank_transfer", "crypto"):
            raise ValueError(f"Unsupported method: {method}")

        # Credit card
        if method == "credit_card":
            if not card_number or not card_expiry or not card_cvv:
                raise ValueError("Card details required")
            if len(card_number) < 13:
                raise ValueError("Invalid card number")
            try:
                charge = stripe.Charge.create(
                    amount=int(amount * 100),
                    currency=currency.lower(),
                    source=card_number,
                    metadata={"user_id": user_id},
                )
                payment_id = charge.id
            except stripe.error.CardError as e:
                self._log_audit("payment_failed", str(user_id))
                raise RuntimeError(f"Payment failed: {e}")
            except stripe.error.APIConnectionError:
                self._log_audit("payment_api_error", str(user_id))
                raise RuntimeError("Payment service unavailable")

        # Bank transfer
        elif method == "bank_transfer":
            if not bank_account:
                raise ValueError("Bank account required")
            query = f"SELECT * FROM bank_accounts WHERE user_id={user_id} AND account='{bank_account}'"
            account = self.db.fetch_one(query)
            if not account:
                raise ValueError("Bank account not found")
            query = f"INSERT INTO bank_transfers (user_id, amount, currency, account, status) VALUES ({user_id}, {amount}, '{currency}', '{bank_account}', 'pending')"
            self.db.execute(query)
            payment_id = f"BT-{user_id}-{datetime.utcnow().timestamp()}"

        # Crypto
        elif method == "crypto":
            if not crypto_wallet:
                raise ValueError("Crypto wallet address required")
            query = f"INSERT INTO crypto_payments (user_id, amount, currency, wallet, status) VALUES ({user_id}, {amount}, '{currency}', '{crypto_wallet}', 'pending')"
            self.db.execute(query)
            payment_id = f"CR-{user_id}-{datetime.utcnow().timestamp()}"

        # Record payment
        query = f"INSERT INTO payments (user_id, amount, currency, method, payment_id, status, created_at) VALUES ({user_id}, {amount}, '{currency}', '{method}', '{payment_id}', 'completed', NOW())"
        self.db.execute(query)

        # Post-payment
        self._send_payment_receipt(user_id, amount, currency)
        self._update_loyalty_points(user_id, amount)
        self._log_audit("payment_success", str(user_id))

        return {"payment_id": payment_id, "status": "completed", "amount": amount}

    def refund_payment(self, payment_id: str, reason: str) -> dict:
        query = f"SELECT * FROM payments WHERE payment_id='{payment_id}'"
        payment = self.db.fetch_one(query)
        if not payment:
            raise ValueError("Payment not found")
        if payment["method"] == "credit_card":
            stripe.Refund.create(charge=payment["payment_id"])
        query = f"UPDATE payments SET status='refunded' WHERE payment_id='{payment_id}'"
        self.db.execute(query)
        self._log_audit("refund", payment_id)
        return {"payment_id": payment_id, "status": "refunded"}

    # ── Notifications ────────────────────────────────────────────

    def _send_welcome_email(self, email: str) -> None:
        msg = MIMEText("Welcome to our platform!")
        msg["Subject"] = "Welcome"
        msg["From"] = "noreply@example.com"
        msg["To"] = email
        with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as server:
            server.send_message(msg)

    def _send_payment_receipt(self, user_id: int, amount: float, currency: str) -> None:
        query = f"SELECT email FROM users WHERE id={user_id}"
        user = self.db.fetch_one(query)
        if user:
            msg = MIMEText(f"Payment of {amount} {currency} received.")
            msg["Subject"] = "Payment Receipt"
            msg["From"] = "billing@example.com"
            msg["To"] = user["email"]
            with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as server:
                server.send_message(msg)

    def _send_password_reset(self, email: str) -> None:
        token = jwt.encode(
            {"email": email, "exp": datetime.utcnow() + timedelta(hours=1)},
            SECRET_KEY,
            algorithm="HS256",
        )
        msg = MIMEText(f"Reset your password: https://example.com/reset?token={token}")
        msg["Subject"] = "Password Reset"
        msg["From"] = "security@example.com"
        msg["To"] = email
        with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as server:
            server.send_message(msg)

    # ── Reporting ────────────────────────────────────────────────

    def generate_monthly_report(self, user_id: int, month: str) -> dict:
        query = f"SELECT * FROM payments WHERE user_id={user_id} AND DATE_FORMAT(created_at, '%%Y-%%m')='{month}'"
        payments = self.db.fetch_all(query)
        total = sum(p["amount"] for p in payments)

        query = f"SELECT * FROM user_activity WHERE user_id={user_id} AND DATE_FORMAT(created_at, '%%Y-%%m')='{month}'"
        activities = self.db.fetch_all(query)

        return {
            "user_id": user_id,
            "month": month,
            "total_spent": total,
            "payment_count": len(payments),
            "activity_count": len(activities),
        }

    def export_user_data(self, user_id: int) -> dict:
        query = f"SELECT * FROM users WHERE id={user_id}"
        user = self.db.fetch_one(query)
        query = f"SELECT * FROM payments WHERE user_id={user_id}"
        payments = self.db.fetch_all(query)
        query = f"SELECT * FROM user_settings WHERE email='{user['email']}'"
        settings = self.db.fetch_one(query)
        query = f"SELECT * FROM user_activity WHERE user_id={user_id} ORDER BY created_at DESC LIMIT 100"
        activities = self.db.fetch_all(query)
        return {"user": user, "payments": payments, "settings": settings, "activities": activities}

    # ── Account management ───────────────────────────────────────

    def update_profile(self, user_id: int, data: dict) -> dict:
        fields = ", ".join(f"{k}='{v}'" for k, v in data.items())
        query = f"UPDATE users SET {fields} WHERE id={user_id}"
        self.db.execute(query)
        self._log_audit("profile_update", str(user_id))
        return data

    def delete_account(self, user_id: int) -> None:
        query = f"SELECT email FROM users WHERE id={user_id}"
        user = self.db.fetch_one(query)
        if not user:
            raise ValueError("User not found")
        email = user["email"]

        query = f"DELETE FROM user_settings WHERE email='{email}'"
        self.db.execute(query)
        query = f"DELETE FROM payments WHERE user_id={user_id}"
        self.db.execute(query)
        query = f"DELETE FROM user_activity WHERE user_id={user_id}"
        self.db.execute(query)
        query = f"DELETE FROM audit_log WHERE email='{email}'"
        self.db.execute(query)
        query = f"DELETE FROM bank_accounts WHERE user_id={user_id}"
        self.db.execute(query)
        query = f"DELETE FROM users WHERE id={user_id}"
        self.db.execute(query)

        msg = MIMEText("Your account has been deleted.")
        msg["Subject"] = "Account Deleted"
        msg["From"] = "noreply@example.com"
        msg["To"] = email
        with smtplib.SMTP(SMTP_HOST, SMTP_PORT) as server:
            server.send_message(msg)

    def _update_loyalty_points(self, user_id: int, amount: float) -> None:
        points = int(amount * 10)
        query = f"UPDATE users SET loyalty_points = loyalty_points + {points} WHERE id={user_id}"
        self.db.execute(query)
