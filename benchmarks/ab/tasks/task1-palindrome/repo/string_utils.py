"""String utilities — contains a bug. See test_string_utils.py for the contract."""


def is_palindrome(s: str) -> bool:
    # BUG: does not normalize case or strip non-alphanumeric characters,
    # so "Racecar" and "A man a plan a canal Panama" are reported as non-palindromes.
    return s == s[::-1]
