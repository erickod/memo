from app.billing.application.usecases.calculate_limit_usecase import CalculateLimitUseCase


class CheckoutUseCase:
    def __init__(self, *, calculate: CalculateLimitUseCase) -> None:
        self._calculate = calculate

    def execute(self) -> int:
        return self._calculate.execute(1, 2)
