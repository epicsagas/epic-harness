import pytest

from stats import mean, moving_average


def test_mean():
    assert mean([1, 2, 3]) == 2.0


def test_moving_average_basic():
    # window=2 over [1,2,3,4] -> consecutive pair averages [1.5, 2.5, 3.5]
    assert moving_average([1, 2, 3, 4], window=2) == [1.5, 2.5, 3.5]


def test_moving_average_window_equals_len():
    assert moving_average([2.0, 4.0], window=2) == [3.0]


def test_moving_average_window_larger_than_data():
    # window larger than the data -> no full window fits -> empty list
    assert moving_average([1, 2], window=5) == []


def test_moving_average_invalid_window_raises():
    with pytest.raises(ValueError):
        moving_average([1, 2, 3], window=0)
    with pytest.raises(ValueError):
        moving_average([1, 2, 3], window=-1)
