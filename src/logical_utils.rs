//! Repetition-code timing, power, and energy accounting.
//!
//! Translated from the Python module, minus `ResCat` support.
//!
//! Provides:
//! - Adiabatic elimination helpers (temporarily set `k_b = margin * k2`)
//! - Stabilization, Zeno, CNOT powers (non–autocat version)
//! - Gate durations
//! - Energies for measurement steps, CNOT, unit cell, and full cycle
use crate::gates::{
    drive_opt, e_d, e_holo, e_m, p_buffer_drive, p_cnot_pump, p_pump, p_z_gate, p_zeno_drive,
    t_cnot, t_fizz, t_holo, t_z_gate,
};
use crate::hardware::Hardware;

/// Default holonomic gate duration constant (`T_g2^holo`).
pub const TG2_HOLO: f64 = 5.0;

/// Default FIZZ measurement duration constant (`T_g2^FIZZ`).
pub const TG2_FIZZ: f64 = 2.0;

/// Default margin for adiabatic elimination: `k_b = margin * k2`.
pub const DEFAULT_MARGIN: f64 = 5.0;

// ---------------------------------------------------------------------
// Power wrappers (non-autocat)
// ---------------------------------------------------------------------

/// Stabilization-only (Id gate) power.
pub fn power_stab(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let ed = e_d(hw, hw.alpha, k2);
    // P_Idgate = pump + buffer
    let result = p_pump(hw, k2, macro_flag) + p_buffer_drive(hw, ed, macro_flag);

    hw.k_b = old_k_b;
    result
}

/// CNOT power (two-qubit gate).
pub fn power_cnot(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let ed = e_d(hw, hw.alpha, k2);
    let gcnot = drive_opt(hw, hw.alpha, k2, crate::gates::DriveInteraction::Cnot);

    // P_Cnotgate = pump + buffer + CNOT pump
    let result = p_pump(hw, k2, macro_flag)
        + p_buffer_drive(hw, ed, macro_flag)
        + p_cnot_pump(hw, gcnot, macro_flag);

    hw.k_b = old_k_b;
    result
}

/// Zeno gate power (includes stabilization).
pub fn power_z(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let ed = e_d(hw, hw.alpha, k2);
    let ez = drive_opt(hw, hw.alpha, k2, crate::gates::DriveInteraction::Z);

    let result = p_pump(hw, k2, macro_flag)
        + p_buffer_drive(hw, ed, macro_flag)
        + p_zeno_drive(hw, ez, macro_flag);

    hw.k_b = old_k_b;
    result
}

/// ATS pump power (two-photon dissipation).
pub fn power_atspump(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let result = p_pump(hw, k2, macro_flag);

    hw.k_b = old_k_b;
    result
}

// ---------------------------------------------------------------------
// Durations
// ---------------------------------------------------------------------

/// Durations of the composite measurement step:
/// `[T_halfZ, T_holo, T_FIZZ]`
#[must_use]
pub fn duration_meas(k2: f64, ez: f64, hw: &Hardware) -> [f64; 3] {
    let t_half_z = t_z_gate(hw.alpha, ez) / 2.0;
    let t_h = t_holo(hw.alpha, k2, TG2_HOLO);
    let t_f = t_fizz(hw.alpha, k2, TG2_FIZZ);
    [t_half_z, t_h, t_f]
}

/// Total durations for one repetition-code cycle:
/// `[T_prep, T_CNOT, T_meas, T_cycle]`
pub fn duration_cycle(k2: f64, hw: &mut Hardware) -> [f64; 4] {
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let ez = drive_opt(hw, hw.alpha, k2, crate::gates::DriveInteraction::Z);
    let gcnot = drive_opt(hw, hw.alpha, k2, crate::gates::DriveInteraction::Cnot);

    let t_meas = duration_meas(k2, ez, hw).iter().sum::<f64>();
    let t_prep = 1.0 / k2;
    let t_cnot = t_cnot(hw.alpha, gcnot);
    let t_cycle = t_prep + 2.0 * t_cnot + t_meas;

    hw.k_b = old_k_b;
    [t_prep, t_cnot, t_meas, t_cycle]
}

// ---------------------------------------------------------------------
// Energies
// ---------------------------------------------------------------------

/// Energies of the measurement substeps: `[E_halfZ, E_holo, E_FIZZ]`
pub fn e_meas(k2: f64, hw: &mut Hardware, macro_flag: bool) -> [f64; 3] {
    // Baseline regime: adiabatic elimination k_b = margin * k2
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let ed = e_d(hw, hw.alpha, k2);
    let ez = drive_opt(hw, hw.alpha, k2, crate::gates::DriveInteraction::Z);
    let e_holo_val = e_holo(hw.alpha, k2);
    let e_m_val = e_m(hw.alpha, k2);

    let p_half_z = p_z_gate(hw, ed, ez, k2, macro_flag);
    let p_holo = p_pump(hw, k2, macro_flag) + p_buffer_drive(hw, e_holo_val, macro_flag);
    let p_fizz = p_z_gate(hw, ed, e_m_val, k2, macro_flag);

    let [t_half_z, t_h, t_f] = duration_meas(k2, ez, hw);

    hw.k_b = old_k_b;
    [p_half_z * t_half_z, p_holo * t_h, p_fizz * t_f]
}

/// Energy of one CNOT gate.
pub fn e_cnot(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let old_k_b = hw.k_b;
    hw.k_b = DEFAULT_MARGIN * k2;

    let gcnot = drive_opt(hw, hw.alpha, k2, crate::gates::DriveInteraction::Cnot);
    let p = power_cnot(k2, hw, macro_flag);
    let t = t_cnot(hw.alpha, gcnot);

    hw.k_b = old_k_b;
    p * t
}

/// Energy of preparation (Id-only stabilization).
pub fn e_prep(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let p = power_stab(k2, hw, macro_flag);
    let t = 0.4 * 1.0 / k2;
    p * t
}

/// Stabilization energy during one unit cell (prep + measurement).
pub fn e_stab(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    let p_stab = power_stab(k2, hw, macro_flag);
    let [t_prep, _, t_meas, _] = duration_cycle(k2, hw);
    p_stab * (t_prep + t_meas)
}

/// Stabilization energy for edge cases.
///
/// Modeled as `P_stab * T_cycle * num_round`.
#[allow(clippy::cast_precision_loss)]
pub fn e_stab_edge(k2: f64, hw: &mut Hardware, num_round: usize, macro_flag: bool) -> f64 {
    let [_, _, _, t_cycle] = duration_cycle(k2, hw);
    let p_stab = power_stab(k2, hw, macro_flag);
    p_stab * t_cycle * num_round as f64
}

/// Energy of a full unit cell:
///
/// `E_unit_cell = 2*CNOT + prep + meas + stab`.
pub fn e_unit_cell(k2: f64, hw: &mut Hardware, macro_flag: bool) -> f64 {
    2.0 * e_cnot(k2, hw, macro_flag)
        + e_prep(k2, hw, macro_flag)
        + e_meas(k2, hw, macro_flag).iter().sum::<f64>()
        + e_stab(k2, hw, macro_flag)
}

/// Total energy of a full repetition code execution:
///
/// `E_total = unit_cell * (dist - 1) * num_round + stab_edge`.
#[allow(clippy::cast_precision_loss)]
pub fn e_tot(
    k1_on_k2: f64,
    hw: &mut Hardware,
    dist: usize,
    num_round: usize,
    macro_flag: bool,
) -> f64 {
    let k2 = hw.k_1 / k1_on_k2;
    let unit = e_unit_cell(k2, hw, macro_flag);
    let edge = e_stab_edge(k2, hw, num_round, macro_flag);
    unit * (dist as f64 - 1.0) * num_round as f64 + edge
}
