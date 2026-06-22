from string_utils import is_palindrome


def test_simple_palindrome():
    assert is_palindrome("racecar") is True


def test_simple_non_palindrome():
    assert is_palindrome("hello") is False


def test_mixed_case():  # FAILS on the current bug
    assert is_palindrome("Racecar") is True


def test_with_spaces_and_punctuation():  # FAILS on the current bug
    assert is_palindrome("A man, a plan, a canal: Panama") is True


def test_empty_string():
    assert is_palindrome("") is True


def test_single_char():
    assert is_palindrome("a") is True
