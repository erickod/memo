from app.billing.infra.repositories.limit_repository import SqlLimitRepository


class ChargeUseCase:
    def execute(self) -> SqlLimitRepository:
        return SqlLimitRepository()
