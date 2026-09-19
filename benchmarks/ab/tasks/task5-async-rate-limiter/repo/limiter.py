import asyncio
import time
from typing import Optional


class TokenBucketLimiter:
    def __init__(self, rate_per_second: float, max_tokens: float):
        if rate_per_second <= 0 or max_tokens <= 0:
            raise ValueError("Rate and max_tokens must be positive")
        self.rate = float(rate_per_second)
        self.max_tokens = float(max_tokens)
        self.tokens = float(max_tokens)
        self.last_refill = time.monotonic()
        # Missing lock initialization or misuse!
        self._lock = asyncio.Lock()

    def _refill(self):
        """Refill tokens based on elapsed time."""
        now = time.monotonic()
        elapsed = now - self.last_refill
        # BUG: Doesn't clamp to max_tokens and fails to update last_refill properly
        added = elapsed * self.rate
        self.tokens = min(self.max_tokens, self.tokens + added)
        self.last_refill = now

    async def acquire(self, tokens: float = 1.0) -> bool:
        """
        Attempt to acquire tokens asynchronously.
        Returns True if tokens acquired, False otherwise.
        BUG: Does not acquire self._lock, causing race conditions in async concurrency!
        """
        # BUG: Race condition under concurrent async calls
        self._refill()
        if self.tokens >= tokens:
            # Simulated async latency to trigger race conditions
            await asyncio.sleep(0.001)
            self.tokens -= tokens
            return True
        return False

    async def wait_for_token(self, tokens: float = 1.0, timeout: Optional[float] = None) -> bool:
        """Wait until tokens are available or timeout expires."""
        start = time.monotonic()
        while True:
            if await self.acquire(tokens):
                return True
            if timeout is not None and (time.monotonic() - start) >= timeout:
                return False
            await asyncio.sleep(0.01)
