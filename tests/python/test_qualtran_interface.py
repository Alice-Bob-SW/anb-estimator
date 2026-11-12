"""Pin the cost table in README.md ("In Qualtran code") to small bloqs.

`count_resources` finds gates by their printed names. If Qualtran renames
one of them, these tests fail instead of the estimates silently dropping.
"""

import numpy as np
import pytest
from qualtran import Bloq, BloqBuilder, CBit, CtrlSpec, QBit
from qualtran.bloqs.basic_gates import CNOT, Hadamard, Toffoli, TwoBitCSwap
from qualtran.bloqs.mcmt import And

from anb_estimator.qualtran_interface import count_resources

# Qualtran accepts a single dtype at runtime, but its type hint only allows a tuple.
C_CNOT = CNOT().controlled(CtrlSpec(qdtypes=CBit()))  # ty: ignore[invalid-argument-type]


def _counts(bloq: Bloq) -> tuple[int, int, int]:
    c = count_resources(bloq)
    return c.qubit_count, c.cx_count, c.ccx_count


def _two_c_cnots() -> Bloq:
    """Two classically controlled CNOTs on the same qubits."""
    bb = BloqBuilder()
    c = bb.add_register("c", CBit())
    a = bb.add_register("a", QBit())
    b = bb.add_register("b", QBit())
    for _ in range(2):
        c, a, b = bb.add(C_CNOT, ctrl2=c, ctrl=a, target=b)
    return bb.finalize(c=c, a=a, b=b)


def _all_gates() -> Bloq:
    """One of each gate in the cost table, with two `C[CNOT]`."""
    bb = BloqBuilder()
    c = bb.add_register("c", CBit())
    a = bb.add_register("a", QBit())
    b = bb.add_register("b", QBit())
    t = bb.add_register("t", QBit())
    a, b = bb.add(CNOT(), ctrl=a, target=b)
    [a, b], t = bb.add(Toffoli(), ctrl=np.array([a, b]), target=t)
    a, b, t = bb.add(TwoBitCSwap(), ctrl=a, x=b, y=t)
    ctrl, anc = bb.add(And(), ctrl=np.array([a, b]))
    a, b = bb.add(And().adjoint(), ctrl=ctrl, target=anc)
    for _ in range(2):
        c, a, b = bb.add(C_CNOT, ctrl2=c, ctrl=a, target=b)
    a = bb.add(Hadamard(), q=a)
    return bb.finalize(c=c, a=a, b=b, t=t)


@pytest.mark.parametrize(
    ("bloq", "name"),
    [
        (CNOT(), "CNOT"),
        (Toffoli(), "Toffoli"),
        (TwoBitCSwap(), "TwoBitCSwap"),
        (C_CNOT, "C[CNOT]"),
        (And(), "And"),
    ],
)
def test_gate_names(bloq, name):
    assert str(bloq) == name


@pytest.mark.parametrize(
    ("bloq", "expected"),
    [
        (CNOT(), (2, 1, 0)),
        (Toffoli(), (3, 0, 1)),
        (TwoBitCSwap(), (3, 2, 1)),
        (And(), (3, 0, 1)),
        (And().adjoint(), (3, 0, 0)),
        (Hadamard(), (1, 0, 0)),
    ],
    ids=str,
)
def test_single_gate_cost(bloq, expected):
    assert _counts(bloq) == expected


def test_c_cnot_counts_as_half_cnot():
    assert _counts(_two_c_cnots())[1:] == (1, 0)


def test_all_gates_add_up():
    # cx: CNOT 1 + TwoBitCSwap 2 + 2 * C[CNOT] 0.5 = 4
    # ccx: Toffoli 1 + TwoBitCSwap 1 + And 1 = 3
    assert _counts(_all_gates())[1:] == (4, 3)
