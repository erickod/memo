class SqlLimitRepository:
    def save(self, limit: int) -> None:
        self._last = limit
