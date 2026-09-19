from decimal import Decimal, ROUND_HALF_UP
from typing import Union


def round_currency(amount: Union[float, Decimal, int]) -> Decimal:
    """Round money amounts to 2 decimal places using standard banker/half-up rounding."""
    d = Decimal(str(amount)) if not isinstance(amount, Decimal) else amount
    return d.quantize(Decimal("0.01"), rounding=ROUND_HALF_UP)


def format_currency(amount: Union[float, Decimal, int], symbol: str = "$") -> str:
    """Format decimal amount into currency string with symbol."""
    rounded = round_currency(amount)
    return f"{symbol}{rounded:.2f}"
