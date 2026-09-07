from math import floor
from warnings import warn

from qualtran import Bloq

from qsharp_alice_bob_resource_estimator._native import (  # ty: ignore[unresolved-import]
    _estimate_logical_counts,
    _estimate_qsharp_file,
)
from qsharp_alice_bob_resource_estimator.dataclass_wrappers import (
    ErrorBudget,
    Estimates,
    FullResults,
    LogicalCounts,
)
from qsharp_alice_bob_resource_estimator.qualtran_interface import count_resources


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
    "You should have a look at the Readme.md for assumptions on the costs of physical gates."
)


def _format_logical_counts_input(logical_counts: LogicalCounts) -> LogicalCounts:
    """
    Make sure that the logical counts are valid (non-negative integers) and convert them to integers if they are given as floats representing integers (e.g., 3.0).
    in: LogicalCounts with potentially non-integer or negative values
    out: LogicalCounts with non-negativeinteger values, or raises ValueError if the input is invalid
    """

    def _to_uint(k: str, val: float) -> int:
        if val < 0:
            raise ValueError(f"{k} must be >= 0")
        if val != floor(val):
            raise ValueError(
                f"{k} must be an integer or a float representing an integer (e.g., 3.0)"
            )
        return floor(val)

    if logical_counts.qubit_count == 0:
        raise ValueError("The number of qubits must be > 0")
    if logical_counts.ccx_count == 0:
        raise ValueError(
            "The number of CCX gates must be > 0"
        )  # Rust panics if the number of factories is 0.

    return LogicalCounts(**{k: _to_uint(k, v) for k, v in logical_counts.as_dict().items()})


def estimate_logical_counts(
    logical_counts: LogicalCounts,
    frontier: bool,
    error_total: float | None = None,
    error_budget: ErrorBudget | None = None,
) -> FullResults:
    """
    Runs the estimation based on logical counts and returns the results as an Estimates class.

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
    _safe_counts = _format_logical_counts_input(logical_counts)

    if not isinstance(frontier, bool):
        raise TypeError("frontier must be a boolean")

    _check_error_inputs(error_total, error_budget)

    estimate, frontier_data = _estimate_logical_counts(
        _safe_counts.qubit_count,
        _safe_counts.cx_count,
        _safe_counts.ccx_count,
        frontier=frontier,
        error_total=error_total,
        error_budget=error_budget,
    )

    frontier_converted = [Estimates.from_rust(e) for e in frontier_data] if frontier else None

    return FullResults(Estimates.from_rust(estimate), frontier_converted, _safe_counts)


def estimate_from_qualtran(
    bloq: Bloq,
    frontier: bool,
    error_total: float | None = None,
    error_budget: ErrorBudget | None = None,
) -> FullResults:
    """
    Runs the Qualtran estimation and returns the results as an EstimatesPy class.

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

    estimate, frontier_data, counts = _estimate_qsharp_file(
        file_path, frontier=frontier, error_total=error_total, error_budget=error_budget
    )
    counts_converted = LogicalCounts.from_rust(counts)

    frontier_converted = [Estimates.from_rust(e) for e in frontier_data] if frontier else None

    return FullResults(Estimates.from_rust(estimate), frontier_converted, counts_converted)
