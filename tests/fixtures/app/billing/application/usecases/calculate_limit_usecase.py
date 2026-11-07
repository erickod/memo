from app.billing.domain.protocols.limit_repository import LimitRepository


class CalculateLimitUseCase:
    def __init__(self, *, repo: LimitRepository) -> None:
        self._repo = repo

    def execute(self, amount, rate: int) -> int:
        # comment kept stable for hash tests
        """Calculate a limit."""
        limit = amount * rate
        self._repo.save(limit)
        return limit
