You are working on an e-commerce calculation engine in the current directory consisting of three modules:
- `cart.py`: Manages shopping cart items and computes subtotals and totals.
- `discounts.py`: Applies discount codes and tier-based promotions.
- `currency.py`: Handles currency formatting and precision rounding.

Currently, several tests in `test_cart.py` fail because:
1. `discounts.py`: Tier-based promotion discounts for VIP users (`calculate_tier_discount`) are not calculating the tiered percentage thresholds correctly.
2. `cart.py`: The `calculate_final_total` does not properly apply the discount before tax calculation.

Your task:
- Fix `calculate_tier_discount` in `discounts.py` and `calculate_final_total` in `cart.py` so that ALL tests in `test_cart.py` pass when run with `pytest`.
- Ensure you do NOT break existing currency rounding or standard item subtotal calculations in `currency.py` and `cart.py` (Regression Integrity).
- Run `pytest` to verify your solution. Do NOT modify `test_cart.py`.
