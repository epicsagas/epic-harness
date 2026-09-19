import asyncio
import time
import pytest
from limiter import TokenBucketLimiter


@pytest.mark.asyncio
async def test_basic_acquire_and_exhaustion():
    limiter = TokenBucketLimiter(rate_per_second=10.0, max_tokens=5.0)
    # Acquire 5 tokens
    for _ in range(5):
        assert await limiter.acquire(1.0) is True
    # 6th should fail immediately
    assert await limiter.acquire(1.0) is False


@pytest.mark.asyncio
async def test_concurrent_acquire_race_condition():
    # 10 initial tokens, rate 10/s
    limiter = TokenBucketLimiter(rate_per_second=10.0, max_tokens=10.0)
    
    # 25 coroutines trying to acquire 1 token concurrently
    tasks = [limiter.acquire(1.0) for _ in range(25)]
    results = await asyncio.gather(*tasks)
    
    # Exactly 10 (or up to 11 with tiny refill during sleep) should succeed, NEVER 25
    success_count = sum(1 for r in results if r is True)
    assert success_count <= 11, f"Expected at most 11 concurrent acquires, but got {success_count}"
    assert limiter.tokens >= 0.0, f"Limiter tokens dropped below zero: {limiter.tokens}"


@pytest.mark.asyncio
async def test_refill_over_time():
    limiter = TokenBucketLimiter(rate_per_second=20.0, max_tokens=5.0)
    # Drain
    assert await limiter.acquire(5.0) is True
    assert await limiter.acquire(1.0) is False
    
    # Sleep 0.2s -> should refill 4.0 tokens
    await asyncio.sleep(0.2)
    assert await limiter.acquire(3.0) is True


@pytest.mark.asyncio
async def test_wait_for_token_timeout():
    limiter = TokenBucketLimiter(rate_per_second=2.0, max_tokens=1.0)
    assert await limiter.acquire(1.0) is True
    # Next token takes 0.5s. Timeout at 0.1s should return False
    res = await limiter.wait_for_token(1.0, timeout=0.1)
    assert res is False
