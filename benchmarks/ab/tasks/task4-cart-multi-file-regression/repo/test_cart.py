from decimal import Decimal
import pytest
from cart import ShoppingCart
from currency import round_currency, format_currency
from discounts import calculate_tier_discount


# ─── PASS_TO_PASS REGRESSION TESTS (Must continue passing) ─────────────────────

def test_currency_rounding_and_formatting():
    assert round_currency(12.345) == Decimal("12.35")
    assert round_currency(12.344) == Decimal("12.34")
    assert format_currency(10.5) == "$10.50"
    assert format_currency(Decimal("100.999")) == "$101.00"


def test_empty_cart_subtotal():
    cart = ShoppingCart()
    assert cart.subtotal() == Decimal("0.00")
    res = cart.calculate_final_total()
    assert res["subtotal"] == Decimal("0.00")
    assert res["total"] == Decimal("0.00")


def test_invalid_item_quantity():
    cart = ShoppingCart()
    with pytest.raises(ValueError, match="positive"):
        cart.add_item("Book", 10.0, quantity=0)
    with pytest.raises(ValueError, match="positive"):
        cart.add_item("Book", 10.0, quantity=-1)


# ─── FAIL_TO_PASS TESTS (Target bugs to fix) ───────────────────────────────────

def test_tier_discounts_thresholds():
    # Bronze: 5% for >= $50
    assert calculate_tier_discount(Decimal("40.00"), "bronze") == Decimal("0.00")
    assert calculate_tier_discount(Decimal("50.00"), "bronze") == Decimal("2.50")
    
    # Silver: 10% for >= $100
    assert calculate_tier_discount(Decimal("90.00"), "silver") == Decimal("0.00")
    assert calculate_tier_discount(Decimal("100.00"), "silver") == Decimal("10.00")
    
    # Gold: 15% for >= $200, 20% for >= $500
    assert calculate_tier_discount(Decimal("150.00"), "gold") == Decimal("0.00")
    assert calculate_tier_discount(Decimal("200.00"), "gold") == Decimal("30.00")
    assert calculate_tier_discount(Decimal("500.00"), "gold") == Decimal("100.00")
    
    # Platinum: 25% for any subtotal > 0
    assert calculate_tier_discount(Decimal("10.00"), "platinum") == Decimal("2.50")


def test_cart_final_total_tax_on_discounted_subtotal():
    # Gold user with $200 subtotal -> 15% discount ($30) -> discounted subtotal = $170
    # Tax rate 10% on $170 = $17.00 -> Grand total = $187.00
    cart = ShoppingCart(user_tier="gold")
    cart.add_item("Headphones", 100.0, quantity=2)
    
    res = cart.calculate_final_total(tax_rate=0.10)
    assert res["subtotal"] == Decimal("200.00")
    assert res["discount"] == Decimal("30.00")
    assert res["tax"] == Decimal("17.00")
    assert res["total"] == Decimal("187.00")
