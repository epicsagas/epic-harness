from decimal import Decimal
from typing import Dict, List, Optional
from currency import round_currency
from discounts import calculate_coupon_discount, calculate_tier_discount


class ShoppingCart:
    def __init__(self, user_tier: str = "bronze"):
        self.user_tier = user_tier
        self.items: List[Dict[str, any]] = []

    def add_item(self, name: str, price: float, quantity: int = 1):
        if quantity <= 0:
            raise ValueError("Quantity must be positive")
        if price < 0:
            raise ValueError("Price cannot be negative")
        self.items.append({"name": name, "price": Decimal(str(price)), "quantity": quantity})

    def subtotal(self) -> Decimal:
        total = sum((item["price"] * item["quantity"] for item in self.items), Decimal("0.00"))
        return round_currency(total)

    def calculate_final_total(self, coupon_code: Optional[str] = None, tax_rate: float = 0.10) -> Dict[str, Decimal]:
        """
        Calculates subtotal, discount, taxable amount, tax, and grand total.
        Formula:
        1. subtotal = sum(price * qty)
        2. discount = max(coupon_discount, tier_discount)
        3. discounted_subtotal = max(0, subtotal - discount)
        4. tax = round_currency(discounted_subtotal * tax_rate)
        5. total = discounted_subtotal + tax
        BUG: Adds tax BEFORE discount, which causes incorrect final totals.
        """
        sub = self.subtotal()
        coupon_disc = calculate_coupon_discount(sub, coupon_code)
        tier_disc = calculate_tier_discount(sub, self.user_tier)
        effective_disc = max(coupon_disc, tier_disc)
        
        # BUG: Tax computed on full subtotal instead of discounted subtotal!
        tax = round_currency(sub * Decimal(str(tax_rate)))
        total = max(Decimal("0.00"), sub + tax - effective_disc)
        
        return {
            "subtotal": sub,
            "discount": round_currency(effective_disc),
            "tax": tax,
            "total": round_currency(total),
        }
