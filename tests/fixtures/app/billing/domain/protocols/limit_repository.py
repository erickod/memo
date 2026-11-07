from typing import Protocol


class LimitRepository(Protocol):
    def save(self, limit: int) -> None: ...
