You are working on an asynchronous microservice rate limiter in the current directory.

The module `limiter.py` implements an asynchronous `TokenBucketLimiter` for controlling API request throughput. Currently, several tests in `test_limiter.py` fail because:
1. Concurrency race condition: When multiple coroutines acquire tokens simultaneously without proper `asyncio.Lock`, tokens drop below zero and exceed the rate capacity.
2. Token refill calculation: The time-based token refill calculation does not clamp to `max_tokens` or drops fractional tokens incorrectly.

Your task:
- Fix `acquire` and `refill` in `limiter.py` so that ALL tests in `test_limiter.py` pass when run with `pytest`.
- Ensure thread-safe/async-safe concurrency synchronization using `asyncio.Lock`.
- Run `pytest` to verify your solution. Do NOT modify `test_limiter.py`.
