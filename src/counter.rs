//! Tools to manipulate counts of logical qubits and gates.
//!
//! Can compute logical space and time overheads for resource estimation from Q#
//! files. Can also directly instantiate a logical count from number of logical
//! qubits, of CX and of CCX.

use std::{fs::read_to_string, path::Path};

use num_bigint::BigUint;
use num_complex::Complex;
use num_traits::ToPrimitive;
use qsc::{
    Backend, BackendResult, LanguageFeatures, SourceMap, TargetCapabilityFlags,
    interpret::{GenericReceiver, Interpreter},
};
use resource_estimator::estimates::{ErrorBudget, Overhead};

/// Count the number of logical qubits, CX and CCX gates.
#[allow(clippy::struct_field_names)]
#[derive(Clone, Default)]
#[must_use]
pub struct LogicalCounts {
    pub(crate) qubit_count: u64,
    pub(crate) cx_count: u64,
    pub(crate) ccx_count: u64,

    free_list: Vec<usize>, // holds indices of allocated qubits
}

impl LogicalCounts {
    /// Create from logical qubits and gates counts.
    #[allow(clippy::similar_names)]
    pub fn new(qubit_count: u64, cx_count: u64, ccx_count: u64) -> Self {
        Self {
            qubit_count,
            cx_count,
            ccx_count,
            free_list: vec![],
        }
    }

    /// Count the logical resources from a Q# file.
    #[allow(clippy::default_trait_access)]
    pub fn from_qsharp(filename: impl AsRef<Path>) -> Result<Self, String> {
        let content = read_to_string(filename).map_err(|_| String::from("Cannot read filename"))?;

        let sources = SourceMap::new([("source".into(), content.into())], None);

        let (std_id, store) = qsc::compile::package_store_with_stdlib(TargetCapabilityFlags::all());
        let mut interpreter = Interpreter::new(
            sources,
            qsc::PackageType::Exe,
            TargetCapabilityFlags::all(),
            LanguageFeatures::default(),
            store,
            &[(std_id, None)],
            Default::default(),
        )
        .map_err(|_| String::from("Cannot create interpreter"))?;

        let mut counter = Self::default();
        let mut stdout = std::io::stdout();
        let mut out = GenericReceiver::new(&mut stdout);

        interpreter
            .eval_entry_with_sim(&mut counter, &mut out)
            .map_err(|_| String::from("Cannot estimate Q# code"))?;

        Ok(counter)
    }
}

impl Overhead for LogicalCounts {
    /// The number of logical qubits to execute the algorithm after mapping.
    ///
    /// This number includes "horizontal" routing qubits for the logical qubits, including the top
    /// one (in between the compute and factory parts). It does not include the "vertical" routing
    /// qubits (they are included only for displaying the estimates results). It does not include
    /// qubit used to produce magic states.
    fn logical_qubits(&self) -> Result<u64, String> {
        let horizontal_routing_qubits = self.qubit_count.div_ceil(2) + 1;

        Ok(self.qubit_count + horizontal_routing_qubits)
    }

    #[allow(clippy::similar_names)]
    fn logical_depth(&self, _: &ErrorBudget) -> Result<u64, String> {
        let cx_f = self.cx_count.to_f64().expect("#CX didn't convert to f64");
        let ccx_f = self.ccx_count.to_f64().expect("#CCX didn't convert to f64");

        // arXiv:2302.06639 (p. 30, Fig. 27); measurement is counted as 0.2
        // cycles according to open source code (5 steps in a cycle)
        let cx_cycles = 2.2;

        // arXiv:2302.06639 (p. 36, Fig. 33); the cost is approximated as 3
        // CNOT (3 * 2.2), then 1.5 CNOT subject to measurement outcome (1.5
        // * 2.2), and measurement (0.2)
        let ccx_cycles = 10.1;

        Ok(((cx_f * cx_cycles) + (ccx_f * ccx_cycles))
            .ceil()
            .to_u64()
            .expect("logical depth is too large"))
    }

    fn num_magic_states(&self, _: &ErrorBudget, _: usize) -> Result<u64, String> {
        Ok(self.ccx_count)
    }
}

impl Backend for LogicalCounts {
    fn ccx(&mut self, _ctl0: usize, _ctl1: usize, _q: usize) -> Result<(), String> {
        self.ccx_count += 1;
        Ok(())
    }

    fn cx(&mut self, _ctl: usize, _q: usize) -> Result<(), String> {
        self.cx_count += 1;
        Ok(())
    }

    fn cy(&mut self, _ctl: usize, _q: usize) -> Result<(), String> {
        self.cx_count += 1;
        Ok(())
    }

    fn cz(&mut self, _ctl: usize, _q: usize) -> Result<(), String> {
        self.cx_count += 1;
        Ok(())
    }

    fn h(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn m(&mut self, _q: usize) -> Result<BackendResult, String> {
        Ok(BackendResult::Val(false))
    }

    fn mresetz(&mut self, _q: usize) -> Result<BackendResult, String> {
        Ok(BackendResult::Val(false))
    }

    fn reset(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn sadj(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn s(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn swap(&mut self, _q0: usize, _q1: usize) -> Result<(), String> {
        self.cx_count += 3;
        Ok(())
    }

    fn x(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn y(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn z(&mut self, _q: usize) -> Result<(), String> {
        Ok(())
    }

    fn qubit_allocate(&mut self) -> Result<usize, String> {
        Ok(if let Some(qubit) = self.free_list.pop() {
            qubit
        } else {
            let qubit = self.qubit_count;
            self.qubit_count += 1;
            qubit.to_usize().expect("qubit is too large")
        })
    }

    fn qubit_release(&mut self, q: usize) -> Result<bool, String> {
        self.free_list.push(q);
        Ok(true)
    }

    fn capture_quantum_state(&mut self) -> Result<(Vec<(BigUint, Complex<f64>)>, usize), String> {
        Ok((vec![], 0))
    }

    fn qubit_is_zero(&mut self, _q: usize) -> Result<bool, String> {
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> ErrorBudget {
        ErrorBudget::new(0.1, 0.1, 0.0)
    }

    #[test]
    /// 2.2 cycles per CX, 10.1 cycles per CCX, rounded up (arXiv:2302.06639,
    /// Fig. 27, Fig. 33).
    fn logical_depth_matches_2_2_cx_plus_10_1_ccx() {
        let cases: [(u64, u64, u64); 4] = [(0, 0, 0), (10, 0, 22), (0, 10, 101), (10, 5, 73)];

        for (cx, ccx, expected) in cases {
            let counts = LogicalCounts::new(0, cx, ccx);
            assert_eq!(
                counts.logical_depth(&budget()).expect("should compute"),
                expected
            );
        }
    }

    #[test]
    fn logical_qubits_overhead_formula() {
        for qubit_count in [0, 1, 2, 3, 40, 2333] {
            let counts = LogicalCounts::new(qubit_count, 0, 0);
            let expected = qubit_count + (qubit_count.div_ceil(2) + 1);
            assert_eq!(counts.logical_qubits().expect("should compute"), expected);
        }
    }

    #[test]
    /// Mirrors the README's "Supported gates and their costs" table.
    fn backend_gate_costs_match_readme_table() {
        let mut counts = LogicalCounts::default();

        let q0 = counts.qubit_allocate().expect("should allocate");
        let q1 = counts.qubit_allocate().expect("should allocate");
        let q2 = counts.qubit_allocate().expect("should allocate");

        counts.ccx(q0, q1, q2).expect("should apply");
        counts.cx(q0, q1).expect("should apply");
        counts.cy(q0, q1).expect("should apply");
        counts.cz(q0, q1).expect("should apply");
        counts.swap(q0, q1).expect("should apply");
        counts.x(q0).expect("should apply");
        counts.y(q0).expect("should apply");
        counts.z(q0).expect("should apply");
        counts.h(q0).expect("should apply");
        counts.sadj(q0).expect("should apply");
        counts.s(q0).expect("should apply");
        let _ = counts.m(q0).expect("should apply");
        let _ = counts.mresetz(q0).expect("should apply");
        counts.reset(q0).expect("should apply");

        assert_eq!(counts.qubit_count, 3);
        assert_eq!(counts.ccx_count, 1);
        // cx + cy + cz (1 each) + swap (3) = 6
        assert_eq!(counts.cx_count, 6);
    }

    #[test]
    fn qubit_allocate_reuses_released_qubits() {
        let mut counts = LogicalCounts::default();
        let q0 = counts.qubit_allocate().expect("should allocate");
        counts.qubit_release(q0).expect("should release");
        let q1 = counts.qubit_allocate().expect("should allocate");

        assert_eq!(q0, q1);
        assert_eq!(counts.qubit_count, 1);
    }
}
