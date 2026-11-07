from app.billing.domain.entities.invoice import Invoice


class CancelInvoiceUseCase:
    def __init__(self, *, invoice: Invoice) -> None:
        self._invoice = invoice

    def execute(self) -> None:
        self._invoice.status = "cancel"
