from typing import Any

# Import Qualtran tools
from qualtran import Bloq
from qualtran.resource_counting import QubitCount, get_cost_value
from qualtran.resource_counting.generalizers import (
    generalize_cvs,
    ignore_alloc_free,
    ignore_split_join,
)

from anb_estimator.dataclass_wrappers import LogicalCounts

default_generalizer = (ignore_alloc_free, ignore_split_join, generalize_cvs)


def _round(value: Any, name: str):
    """Round, but annotate if there is an error."""
    try:
        return round(value)
    except Exception as e:
        e.add_note(f"{name} count not convertible to integer (symbolic?); value: {value!r}")
        raise


def count_resources(
    bloq: Bloq, graph_generalizer: tuple[Any, ...] = default_generalizer
) -> LogicalCounts:
    """Count the number of qubits, cx and ccx required for a given qualtran Bloq.

    -We count classically controlled CNOT as half a CNOT
    -TwoBitCSwap are not native to A&B architectures
        and are decomposed in 2 CNOT + 1 Toffoli
    -And gates are counted as a Toffoli,
        And.adjoint() are not counted, so the pair of Ands counts as 1 Toffoli in Gidney's adder,
        see arXiv:2302.06639 G.2
    -Single qubit gates are not counted

    Parameters
    ----------
    bloq : Bloq
        Bloq of the circuit for which we want to count resources

    Returns
    -------
    num_qubits : int
        number of qubits needed for the Bloq
    num_cx : int
        number of cx needed for the Bloq
    num_ccx : int
        number of ccx needed for the Bloq

    """
    num_qubits = _round(get_cost_value(bloq, QubitCount()), "Qubit")

    _, sigma = bloq.call_graph(graph_generalizer)
    dict_sigma = {str(k): v for k, v in sigma.items()}

    num_cx = 0
    num_ccx = 0
    if "CNOT" in dict_sigma:
        num_cx += dict_sigma["CNOT"]
    if "TwoBitCSwap" in dict_sigma:  # needs to be decomposed on A&B architecture
        num_cx += 2 * dict_sigma["TwoBitCSwap"]
        num_ccx += dict_sigma["TwoBitCSwap"]
    if "C[CNOT]" in dict_sigma:
        num_cx += 0.5 * dict_sigma["C[CNOT]"]

    if "Toffoli" in dict_sigma:  # needs to be decomposed on A&B architecture
        num_ccx += dict_sigma["Toffoli"]
    if "And" in dict_sigma:
        num_ccx += dict_sigma["And"]

    num_cx = _round(num_cx, "CX")
    num_ccx = _round(num_ccx, "CCX")

    return LogicalCounts(qubit_count=num_qubits, cx_count=num_cx, ccx_count=num_ccx)
