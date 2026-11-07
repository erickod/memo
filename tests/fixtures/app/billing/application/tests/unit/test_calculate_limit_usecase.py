from app.billing.application.usecases.calculate_limit_usecase import CalculateLimitUseCase
from app.billing.infra.repositories.limit_repository import SqlLimitRepository


def test_given_amount_when_calculate_then_returns_limit():
    usecase = CalculateLimitUseCase(repo=SqlLimitRepository())
    assert usecase.execute(2, 3) == 6
