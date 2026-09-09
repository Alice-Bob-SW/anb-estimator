//! Cat-qubit power, cabling, and noise utilities.
//!
//! This module provides:
//! - Cryostat cabling models (via `Hardware`'s `Cabling` field)
//! - Drive/interaction strength helpers and gate-time formulas
//! - Power accounting for pump/drive/readout
//! - Simple noise models for holonomic + FIZZ gates
//!
//! All temperatures are in Kelvin, frequencies in angular units (rad/s),
//! and powers/energies follow the conventions in the associated report/notes.

use statrs::function::erf::erf;
use std::f64::consts::PI;

use crate::hardware::Hardware;

/// Default fit parameters for holonomic gate error model (Eq. S6).
pub const PARAM_FIT_HOLO: (f64, f64, f64) = (0.2, 2.6, 1.2);

// ---------------------------------------------------------------------
// Drive amplitude helpers & gate durations
// ---------------------------------------------------------------------

/// Buffer drive formula: `e_d(H, alpha, k2)`.
#[must_use]
pub fn e_d(h: &Hardware, alpha: f64, k2: f64) -> f64 {
    (h.k_b * k2).sqrt() / 2.0 * (alpha * alpha + h.k_1 / (2.0 * k2))
}

/// Zeno drive to reach a given fidelity (report formula after Table 2).
#[must_use]
pub fn e_z(h: &Hardware, alpha: f64, p_z: f64, k2: f64) -> f64 {
    let pref = 2.0 / PI * alpha.powi(3) * p_z * k2 / (1.0 + 2.0 * h.nthb);

    let inside_sqrt = 1.0
        - (PI * PI) * h.k_1 / (4.0 * alpha * alpha * p_z * p_z * k2)
            * (1.0 + 2.0 * h.ntha)
            * (1.0 + 2.0 * h.nthb);

    pref * (1.0 - inside_sqrt.sqrt())
}

/// Pump drive for a CNOT (report formula after Table 2).
#[must_use]
pub fn g_cnot(h: &Hardware, alpha: f64, p_zc: f64, k2: f64) -> f64 {
    let pref = 2.0 / PI * alpha * p_zc * k2 / (1.0 + 2.0 * h.nthb);

    let inside_sqrt = 1.0
        - (PI * PI) * h.k_1 / (4.0 * p_zc * p_zc * k2)
            * (1.0 + 2.0 * h.ntha)
            * (1.0 + 2.0 * h.nthb);

    pref * (1.0 - inside_sqrt.sqrt())
}

/// Interaction type for `drive_opt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveInteraction {
    /// CNOT-type interaction.
    Cnot,
    /// Z-type (Zeno) interaction.
    Z,
}

/// Drive required for maximum-fidelity gates (Table 3).
#[must_use]
pub fn drive_opt(h: &Hardware, alpha: f64, k2: f64, interaction: DriveInteraction) -> f64 {
    let factor = ((1.0 + 2.0 * h.ntha) / (1.0 + 2.0 * h.nthb)).sqrt();
    match interaction {
        DriveInteraction::Cnot => alpha * (h.k_1 * k2).sqrt() * factor,
        DriveInteraction::Z => alpha * alpha * (h.k_1 * k2).sqrt() * factor,
    }
}

/// Holonomic: `e_holo = alpha^2 * g2 = alpha^3 * k2`.
#[must_use]
pub fn e_holo(alpha: f64, k2: f64) -> f64 {
    alpha.powi(3) * k2
}

/// Measurement: `e_m = 2 * alpha^2 * k2`.
#[must_use]
pub fn e_m(alpha: f64, k2: f64) -> f64 {
    2.0 * alpha * alpha * k2
}

/// Z gate time: `T_Zgate = π / (4 α e_z)`.
#[must_use]
pub fn t_z_gate(alpha: f64, e_z: f64) -> f64 {
    PI / (4.0 * alpha * e_z)
}

/// CNOT gate time: `T_CNOT = π / (4 α g_CNOT)`.
#[must_use]
pub fn t_cnot(alpha: f64, g_cnot: f64) -> f64 {
    PI / (4.0 * alpha * g_cnot)
}

/// Holonomic gate time: `T_holo = T_g2 / (α k2)`.
#[must_use]
pub fn t_holo(alpha: f64, k2: f64, t_g2: f64) -> f64 {
    t_g2 / (alpha * k2)
}

/// FIZZ measurement gate time: `T_FIZZ = T_g2 / (α k2)`.
#[must_use]
pub fn t_fizz(alpha: f64, k2: f64, t_g2: f64) -> f64 {
    t_g2 / (alpha * k2)
}

// ---------------------------------------------------------------------
// Power accounting
// ---------------------------------------------------------------------

/// Power to drive two-photon dissipation with ATS.
#[must_use]
pub fn p_pump(h: &Hardware, k2: f64, macro_factor: bool) -> f64 {
    let base = h.p() * h.k_b * k2 / 4.0;
    if macro_factor { h.mp() * base } else { base }
}

/// Power for the buffer drive.
#[must_use]
pub fn p_buffer_drive(h: &Hardware, e_d: f64, macro_factor: bool) -> f64 {
    let base = h.d() * e_d * e_d;
    if macro_factor { h.md() * base } else { base }
}

/// Power for the Zeno drive.
#[must_use]
pub fn p_zeno_drive(h: &Hardware, e_z: f64, macro_factor: bool) -> f64 {
    let base = h.z_factor() * e_z * e_z;
    if macro_factor { h.mz() * base } else { base }
}

/// Power for the CNOT pump.
#[must_use]
pub fn p_cnot_pump(h: &Hardware, g_cnot: f64, macro_factor: bool) -> f64 {
    let base = h.c() * g_cnot * g_cnot;
    if macro_factor { h.mp() * base } else { base }
}

/// Power for stabilisation-only (Id gate).
#[must_use]
pub fn p_id_gate(h: &Hardware, e_d: f64, k2: f64, macro_factor: bool) -> f64 {
    p_pump(h, k2, macro_factor) + p_buffer_drive(h, e_d, macro_factor)
}

/// Power for Zeno gate (includes stabilisation).
#[must_use]
pub fn p_z_gate(h: &Hardware, e_d: f64, e_z: f64, k2: f64, macro_factor: bool) -> f64 {
    p_pump(h, k2, macro_factor)
        + p_buffer_drive(h, e_d, macro_factor)
        + p_zeno_drive(h, e_z, macro_factor)
}

/// Power for both qubits in a CNOT (control stabilised + target ATS).
#[must_use]
pub fn p_cnot_gate(h: &Hardware, e_d: f64, g_cnot: f64, k2: f64, macro_factor: bool) -> f64 {
    p_pump(h, k2, macro_factor)
        + p_buffer_drive(h, e_d, macro_factor)
        + p_cnot_pump(h, g_cnot, macro_factor)
}

// ---------------------------------------------------------------------
// Noise model & circuit helpers
// ---------------------------------------------------------------------

/// Holonomic gate fidelity and duration (Eq. S6, Gautier & Ruiz note).
///
/// Returns `(fidelity, duration)`.
#[must_use]
#[allow(clippy::many_single_char_names)]
pub fn fid_holo(
    k2: f64,
    h: &Hardware,
    alpha: f64,
    param_fit_holo: Option<(f64, f64, f64)>,
    t_g2: f64,
) -> (f64, f64) {
    let (a, b, c) = param_fit_holo.unwrap_or(PARAM_FIT_HOLO);
    let g2 = alpha * k2;
    let t = t_g2 / g2;

    let err = a * alpha.powf(b) * (-c * g2 * t).exp() + 0.5 * h.k_1 * alpha * alpha * t;
    (1.0 - 2.0 * err, t)
}

/// FIZZ measurement fidelity and duration.
///
/// Returns `(fidelity, duration)`.
#[must_use]
pub fn fid_fizz(k2: f64, h: &Hardware, alpha: f64, t_g2: f64) -> (f64, f64) {
    let g2 = alpha * k2;
    let t = t_g2 / g2;
    let snr = (16.0 * h.eta * alpha * g2 * t).sqrt();
    (erf(0.5 * snr), t)
}

/// Combined noise for holonomic + measurement sequence.
///
/// Returns `(error_tot_meas, error_stab_max)`.
#[must_use]
pub fn noise_holo_meas(
    k1_on_k2: f64,
    k2: f64,
    h: &Hardware,
    alpha: f64,
    param_fit_holo: Option<(f64, f64, f64)>,
    t_g2_holo: f64,
    t_g2_fizz: f64,
) -> (f64, f64) {
    // Half-Z error from ratio k1/k2.
    let half_z =
        PI / (4.0 * alpha) * k1_on_k2.sqrt() * ((1.0 + 2.0 * h.ntha) * (1.0 + 2.0 * h.nthb)).sqrt();

    let (holo_fid, t_h) = fid_holo(k2, h, alpha, param_fit_holo, t_g2_holo);
    let (fizz_fid, t_f) = fid_fizz(k2, h, alpha, t_g2_fizz);

    let fid_tot = (1.0 - 2.0 * half_z) * holo_fid * fizz_fid;
    let error_tot_meas = (1.0 - fid_tot) / 2.0;

    let stab_half_z =
        PI / (8.0 * alpha) * k1_on_k2.sqrt() * ((1.0 + 2.0 * h.ntha) * (1.0 + 2.0 * h.nthb)).sqrt();

    let error_stab_mx = stab_half_z + alpha * alpha * h.k_1 * (t_h + t_f) * (1.0 + 2.0 * h.ntha);

    (error_tot_meas, error_stab_mx)
}
