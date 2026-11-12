import warnings
from pathlib import Path

import pytest
from qualtran.bloqs.basic_gates import Toffoli

from anb_estimator import (
    ErrorBudget,
    FullResults,
    LogicalCounts,
    estimate_from_qualtran,
    estimate_logical_counts,
    estimate_qsharp_file,
)

ADDER_QS = str(Path(__file__).parents[2] / "qsharp" / "Adder.qs")
COUNTS = LogicalCounts(40, 10000, 10000)
README_WARNING = "You should have a look at the README.md"


def _estimate_counts(**kwargs) -> FullResults:
    return estimate_logical_counts(COUNTS, **kwargs)


def _estimate_qualtran(**kwargs) -> FullResults:
    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", README_WARNING)
        return estimate_from_qualtran(Toffoli(), **kwargs)


def _estimate_qsharp(**kwargs) -> FullResults:
    with warnings.catch_warnings():
        warnings.filterwarnings("ignore", README_WARNING)
        return estimate_qsharp_file(ADDER_QS, **kwargs)


ALL_ENTRY_POINTS = pytest.mark.parametrize(
    "estimate", [_estimate_counts, _estimate_qualtran, _estimate_qsharp]
)


def test_readme_warning():
    with pytest.warns(UserWarning, match=README_WARNING):
        estimate_from_qualtran(Toffoli(), frontier=False, error_total=0.1)
    with pytest.warns(UserWarning, match=README_WARNING):
        estimate_qsharp_file(ADDER_QS, frontier=False, error_total=0.1)


# --- Smoke tests: each public function accepts every argument ---


@ALL_ENTRY_POINTS
@pytest.mark.parametrize("frontier", [False, True])
def test_smoke_error_total(estimate, frontier):
    result = estimate(frontier=frontier, error_total=0.1)
    assert result.estimates.physical_qubits > 0
    assert (result.frontier is not None) == frontier


@ALL_ENTRY_POINTS
def test_smoke_error_budget(estimate):
    result = estimate(frontier=False, error_budget=ErrorBudget(0.05, 0.05, 0.0))
    assert result.estimates.physical_qubits > 0


@ALL_ENTRY_POINTS
@pytest.mark.parametrize("frontier", [False, True])
def test_smoke_max_physical_qubits(estimate, frontier):
    uncapped = estimate(frontier=False, error_total=0.1)
    cap = uncapped.estimates.physical_qubits
    result = estimate(frontier=frontier, error_total=0.1, max_physical_qubits=cap)
    assert result.estimates.physical_qubits <= cap
    if frontier:
        assert all(e.physical_qubits <= cap for e in result.frontier)


@ALL_ENTRY_POINTS
def test_max_physical_qubits_too_small(estimate):
    with pytest.raises(ValueError, match="too small"):
        estimate(frontier=False, error_total=0.1, max_physical_qubits=1)


def test_default_error_budget_warns():
    with pytest.warns(UserWarning, match="No error budget provided"):
        estimate_logical_counts(COUNTS, frontier=False)


def test_to_json():
    assert '"physical_qubits"' in _estimate_counts(frontier=False, error_total=0.1).to_json()


# --- Input checks ---


@pytest.mark.parametrize(
    ("kwargs", "match"),
    [
        ({"error_total": 0.1, "error_budget": ErrorBudget(0.1, 0.1, 0.0)}, "Exactly one"),
        ({"error_total": -0.1}, "error_total must be between 0 and 1"),
        ({"error_total": 1.5}, "error_total must be between 0 and 1"),
        ({"error_budget": (0.1, 0.1)}, "error_budget must be a 3-tuple"),
        ({"error_budget": ErrorBudget(0.1, -0.1, 0.0)}, "entries must be between 0 and 1"),
        ({"error_budget": ErrorBudget(0.1, 0.1, 1.5)}, "entries must be between 0 and 1"),
    ],
)
def test_error_inputs(kwargs, match):
    with pytest.raises(ValueError, match=match):
        estimate_logical_counts(COUNTS, frontier=False, **kwargs)


@pytest.mark.parametrize(
    ("value", "error"),
    [
        (True, TypeError),
        (1000.0, TypeError),
        ("1000", TypeError),
        (0, ValueError),
        (-1, ValueError),
    ],
)
@ALL_ENTRY_POINTS
def test_max_physical_qubits_input(estimate, value, error):
    with pytest.raises(error, match="max_physical_qubits"):
        estimate(frontier=False, error_total=0.1, max_physical_qubits=value)


@pytest.mark.parametrize(
    ("counts", "match"),
    [
        (LogicalCounts(0, 10, 10), "number of qubits"),
        (LogicalCounts(10, 10, 0), "number of CCX"),
    ],
)
def test_logical_counts_zero(counts, match):
    with pytest.raises(ValueError, match=match):
        estimate_logical_counts(counts, frontier=False, error_total=0.1)


@pytest.mark.parametrize(
    ("args", "match"),
    [((-1, 10, 10), "qubit_count must be >= 0"), ((10, 1.5, 10), "cx_count must be an integer")],
)
def test_logical_counts_constructor(args, match):
    with pytest.raises(ValueError, match=match):
        LogicalCounts(*args)


def test_logical_counts_accepts_integer_floats():
    counts = LogicalCounts(3.0, 4.0, 5.0)
    assert (counts.qubit_count, counts.cx_count, counts.ccx_count) == (3, 4, 5)


def test_frontier_must_be_bool():
    with pytest.raises(TypeError, match="frontier"):
        estimate_logical_counts(COUNTS, frontier=1, error_total=0.1)  # ty: ignore[invalid-argument-type]
    with pytest.raises(TypeError, match="frontier"):
        estimate_qsharp_file(ADDER_QS, frontier=1, error_total=0.1)  # ty: ignore[invalid-argument-type]


def test_qsharp_file_path():
    with pytest.raises(TypeError, match="file_path"):
        estimate_qsharp_file(Path(ADDER_QS), frontier=False)  # ty: ignore[invalid-argument-type]
    with pytest.raises(ValueError, match=r"\.qs"):
        estimate_qsharp_file("Adder.txt", frontier=False)


def test_qualtran_needs_bloq():
    with pytest.raises(TypeError, match="Bloq"):
        estimate_from_qualtran("Toffoli", frontier=False)  # ty: ignore[invalid-argument-type]
