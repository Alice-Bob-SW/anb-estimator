from typing import NamedTuple
from warnings import warn

from qualtran import Bloq

from anb_estimator._native import (  # ty: ignore[unresolved-import]
    FullResults,
    LogicalCounts,
    _estimate_logical_counts,
    _estimate_qsharp_file,
)
from anb_estimator.qualtran_interface import count_resources


class ErrorBudget(NamedTuple):
    """Failure probabilities for one error-budget allocation"""

    p_logical_error: float  # proba of >= 1 logical error
    p_faulty_magic_state_distillation: float  # proba of >= 1 faulty magic state distillation
    p_failed_rotation_synthesis: float  # proba of >= 1 failed rotation synthesis


def _check_error_inputs(error_total: float | None, error_budget: ErrorBudget | None) -> None:
    """
    Ensure that exactly one of `error_total` or `error_budget` is set, and that they are non-negative.
    """
    if error_total is None and error_budget is None:
        warn(
            "No error budget provided. Falling back to default error budget "
            "(0.333 * 0.5, 0.333 * 0.5, 0.0).\n"
        )
    elif error_total is not None and error_budget is not None:
        raise ValueError("Exactly one of error_total or error_budget must be set")
    elif error_total is not None and not 0 <= error_total <= 1:
        raise ValueError("error_total must be between 0 and 1")
    elif error_budget is not None:
        if not len(error_budget) == 3:
            raise ValueError(
                "error_budget must be a 3-tuple (Proba of >= 1 logical error, Proba of >= 1 faulty magic state distillation, Proba of >= 1 failed rotation synthesis)"
            )
        if not all(0 <= x <= 1 for x in error_budget):
            raise ValueError("error_budget entries must be between 0 and 1")


ARBITRARY_CIRCUIT_WARN = (
    "You should have a look at the README.md for assumptions on the costs of physical gates."
)


def _check_logical_counts(logical_counts: LogicalCounts) -> LogicalCounts:
    """
    Ensure the logical counts are usable for Toffoli-factory estimation.

    `LogicalCounts` itself already guarantees non-negative integer fields
    (its constructor validates that); this only checks the additional
    business constraints that this specific estimation needs.
    """
    if logical_counts.qubit_count == 0:
        raise ValueError("The number of qubits must be > 0")
    if logical_counts.ccx_count == 0:
        raise ValueError(
            "The number of CCX gates must be > 0"
        )  # Rust panics if the number of factories is 0.

    return logical_counts


def estimate_logical_counts(
    logical_counts: LogicalCounts,
    frontier: bool,
    error_total: float | None = None,
    error_budget: ErrorBudget | None = None,
) -> FullResults:
    """
    Runs the estimation based on logical counts and returns the results as a FullResults dataclass.

    Args:
        logical_counts (LogicalCounts): Logical counts of the circuit consisting of::
            qubit_count (int): Logical (algorithm) qubit count.
            cx_count (int): Logical CX-equivalent two-qubit gate count.
            ccx_count (int): Logical CCX (Toffoli) gate count.
        frontier (bool): If `true`, also return a list representing the frontier.
        error_total (float): Overall error target; mutually exclusive with `error_budget`.
        error_budget (Tuple): Tuple `(Proba of >= 1 logical error, Proba of >= 1 faulty magic state distillation,
                                Proba of >= 1 failed rotation synthesis)` for an explicit split; mutually exclusive with "error_total".

    Returns:
        FullResults: The estimation results as an FullResults dataclass.
    """
    # --- validate inputs ---
    _safe_counts = _check_logical_counts(logical_counts)

    if not isinstance(frontier, bool):
        raise TypeError("frontier must be a boolean")

    _check_error_inputs(error_total, error_budget)

    return _estimate_logical_counts(
        _safe_counts.qubit_count,
        _safe_counts.cx_count,
        _safe_counts.ccx_count,
        frontier=frontier,
        error_total=error_total,
        error_budget=error_budget,
    )


def estimate_from_qualtran(
    bloq: Bloq,
    frontier: bool,
    error_total: float | None = None,
    error_budget: ErrorBudget | None = None,
) -> FullResults:
    """
    Runs the Qualtran estimation and returns the results as a FullResults dataclass.

    Args:
        bloq (Bloq): The Bloq to be estimated.
        frontier (bool): If `true`, also return a list representing the frontier.
        error_total (float): Overall error target; mutually exclusive with `error_budget`.
        error_budget (Tuple): Tuple `(Proba of >= 1 logical error, Proba of >= 1 faulty magic state distillation,
                                Proba of >= 1 failed rotation synthesis)` for an explicit split; mutually exclusive with "error_total".

    Returns:
        FullResults: The estimation results as an FullResults dataclass.
    """
    # --- validate bloq ---
    if not isinstance(bloq, Bloq):
        raise TypeError("bloq must be a qualtran Bloq")

    # Try resource counting early to ensure the bloq is well-formed
    try:
        logical_count = count_resources(bloq)
    except Exception as exc:
        raise AssertionError("bloq is not a valid qualtran Bloq") from exc

    warn(ARBITRARY_CIRCUIT_WARN)

    return estimate_logical_counts(
        logical_count, frontier=frontier, error_total=error_total, error_budget=error_budget
    )


def estimate_qsharp_file(
    file_path: str,
    frontier: bool,
    error_total: float | None = None,
    error_budget: ErrorBudget | None = None,
) -> FullResults:
    """
    Runs the estimation for a Q# file and returns the results as a dataclass.

    Args:
        file_path (str): The path to the Q# file to be estimated.
        frontier (bool): If `true`, also compute a frontier of estimates (e.g., different distances/α).
        error_total (float): Overall error target; mutually exclusive with `error_budget`.
        error_budget (Tuple): Tuple `(Proba of >= 1 logical error, Proba of >= 1 faulty magic state distillation,
                                Proba of >= 1 failed rotation synthesis)` for an explicit split; mutually exclusive with "error_total".

    Returns:
        FullResults: The estimation results as an FullResults dataclass.
    """
    # --- validate inputs ---
    if not isinstance(file_path, str):
        raise TypeError("file_path must be a string")
    if not file_path.endswith(".qs"):
        raise ValueError("file_path must point to a Q# .qs file")

    if not isinstance(frontier, bool):
        raise TypeError("frontier must be a boolean")

    warn(ARBITRARY_CIRCUIT_WARN)

    _check_error_inputs(error_total, error_budget)

    return _estimate_qsharp_file(
        file_path, frontier=frontier, error_total=error_total, error_budget=error_budget
    )
