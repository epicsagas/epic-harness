from decimal import Decimal
from typing import Optional


def calculate_coupon_discount(subtotal: Decimal, coupon_code: Optional[str]) -> Decimal:
    """Fixed coupon discount calculation."""
    if not coupon_code:
        return Decimal("0.00")
    code = coupon_code.strip().upper()
    if code == "SAVE10":
        return min(subtotal, Decimal("10.00"))
    if code == "PERCENT20":
        return subtotal * Decimal("0.20")
    return Decimal("0.00")


def calculate_tier_discount(subtotal: Decimal, user_tier: str) -> Decimal:
    """
    Tier-based discount:
    - 'bronze': 5% for subtotal >= $50
    - 'silver': 10% for subtotal >= $100
    - 'gold': 15% for subtotal >= $200, 20% for subtotal >= $500
    - 'platinum': 25% for any subtotal > 0
    BUG: Logic is flawed: missing threshold checks and incorrect percentage assignments.
    """
    tier = (user_tier or "bronze").lower()
    # BUG: Calculates flat rate without checking subtotal thresholds
    if tier == "gold":
        return subtotal * Decimal("0.10")  # BUG: should be 0.15 for >=200, 0.20 for >=500
    elif tier == "silver":
        return subtotal * Decimal("0.05")  # BUG: should be 0.10 for >= 100
    elif tier == "platinum":
        return subtotal * Decimal("0.25")
    elif tier == "bronze":
        return Decimal("0.00")  # BUG: should be 0.05 for >= 50
    return Decimal("0.00")
