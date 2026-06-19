"""Basic statistics helpers.

TODO: implement `moving_average(values, window)` — its full contract
(including edge cases and error handling) is specified by test_stats.py,
which currently fails because the function does not exist.
"""


def mean(values):
    return sum(values) / len(values) if values else 0.0
