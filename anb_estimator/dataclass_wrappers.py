import json
from dataclasses import asdict, dataclass
from typing import Any, NamedTuple

from anb_estimator._native import (  # ty: ignore[unresolved-import]
    EstimatesReport,
    LogicalCountsPy,
)


@dataclass(frozen=True, slots=True)
class LogicalCounts:
    qubit_count: int
    cx_count: int
    ccx_count: int

    @classmethod
    def from_rust(cls, inner: LogicalCountsPy) -> "LogicalCounts":
        return cls(
            qubit_count=inner.qubit_count, cx_count=inner.cx_count, ccx_count=inner.ccx_count
        )

    def as_dict(self) -> dict[str, int]:
        return asdict(self)


@dataclass(frozen=True, slots=True)
class Estimates:
    physical_qubits: int
    runtime_seconds: float
    runtime_hours: float
    total_error: float

    code_distance: int
    code_alpha2: float

    factories: int
    factories_distance: int
    factories_alpha2: float

    factory_fraction_percent: float
    factory_fraction: float

    @classmethod
    def from_rust(cls, inner: EstimatesReport) -> "Estimates":
        return cls(
            physical_qubits=inner.physical_qubits,
            runtime_seconds=inner.runtime_seconds,
            runtime_hours=inner.runtime_hours,
            total_error=inner.total_error,
            code_distance=inner.code_distance,
            code_alpha2=inner.code_alpha2,
            factories=inner.factories,
            factories_distance=inner.factories_distance,
            factories_alpha2=inner.factories_alpha2,
            factory_fraction_percent=inner.factory_fraction_percent,
            factory_fraction=inner.factory_fraction,
        )

    def as_dict(self) -> dict[str, float | int]:
        return asdict(self)


@dataclass(frozen=True)
class FullResults:
    estimates: Estimates
    frontier: list[Estimates] | None
    counts: LogicalCounts

    def as_dict(self) -> dict[str, Any]:
        return asdict(self)

    def to_json(self, **kwargs: Any) -> str:
        return json.dumps(self.as_dict(), **kwargs)


class ErrorBudget(NamedTuple):
    """Failure probabilities for one error-budget allocation"""

    p_logical_error: float  # proba of >= 1 logical error
    p_faulty_magic_state_distillation: float  # proba of >= 1 faulty magic state distillation
    p_failed_rotation_synthesis: float  # proba of >= 1 failed rotation synthesis
