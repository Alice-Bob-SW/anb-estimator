//!Cat-qubit power, cabling, and noise utilities.
//!This module provides:
//!- Cryostat cabling models (attenuation)
//!- Hardware parameter container with derived coefficients
//!
//!All temperatures are in Kelvin, frequencies in angular units (rad/s),
//!and powers/energies follow the conventions in the associated report/notes.

use std::collections::HashMap;
use std::f64::consts::PI;

/// Frequency constants (radians per second)
pub const TWOPI: f64 = 2.0 * PI;
/// Base frequency unit (1 Hz in angular units).
pub const HZ: f64 = TWOPI;
/// 1 kHz in angular units.
pub const KHZ: f64 = TWOPI * 1.0e3;
/// 1 MHz in angular units.
pub const MHZ: f64 = TWOPI * 1.0e6;
/// 1 GHz in angular units.
pub const GHZ: f64 = TWOPI * 1.0e9;

// Time constants
/// Microsecond in seconds.
pub const US: f64 = 1.0e-6;
/// Nanosecond in seconds.
pub const NS: f64 = 1.0e-9;

// Unit constants
/// Picohenry in henry.
pub const PH: f64 = 1.0e-12;

// Physical constants
/// Reduced Planck constant ℏ [J·s].
pub const HBAR: f64 = 1e-34;
/// Boltzmann constant `k_B` [J/K].
pub const KB: f64 = 1.38e-23;

/// Cryostat stage temperatures [K], ordered cold → warm.
/// Order: MXC, 100 mK, Still, 4 K, 50 K.
pub const STAGE_TEMPS: [f64; 5] = [0.012, 0.1, 0.97, 3.34, 35.2];
/// External / room temperature [K].
pub const T_EXT: f64 = 300.0;

/// Utility function converting dB to linear
#[must_use]
pub fn db_to_val(db: f64) -> f64 {
    10f64.powf(db / 10.0)
}

/// Logical type of line in the cryostat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineType {
    /// Drive line (qubit drive / readout, etc).
    Drive,
    /// Pump line (parametric drive).
    Pump,
    /// Flux / fast-flux line.
    Ffl,
    /// DC line (slow bias, wiring, etc).
    Dc,
}

/// One physical line from MXC up to 50K/room temperature.
#[derive(Debug, Clone)]
pub struct Line {
    /// Logical type of line (drive/pump/ffl/dc).
    pub line_type: LineType,

    /// Cable technology per stage span; length matches number of spans.
    /// Stage ordering (cold → warm): MXC → 100mK → Still → 4K → 50K
    pub technology: Vec<String>,

    /// Per-span lengths [m], ordered cold → warm.
    pub lengths: Vec<f64>,

    /// Optional fixed pads per stage [dB].
    pub fixed_pads_db: Option<Vec<f64>>,

    /// Keys into the cable DB for different technologies.
    pub cable_db_keys: HashMap<String, String>,
}

impl Default for Line {
    fn default() -> Self {
        let lengths = vec![165.88e-3, 161.58e-3, 248.64e-3, 304.04e-3, 219.29e-3];

        let mut cable_db_keys = HashMap::new();
        cable_db_keys.insert("coax".to_string(), "SC-86/50-SCN-CN".to_string());
        cable_db_keys.insert("dc_PhBr".to_string(), "PhBr_36AWG".to_string());
        cable_db_keys.insert("dc_NbTi".to_string(), "NbTi_36AWG".to_string());

        let line_type = LineType::Drive;
        // Mirrors __post_init__ default for non-DC lines
        let technology = vec![
            "dense".to_string(),
            "dense".to_string(),
            "dense".to_string(),
            "coax".to_string(),
            "coax".to_string(),
        ];

        Self {
            line_type,
            technology,
            lengths,
            fixed_pads_db: None,
            cable_db_keys,
        }
    }
}

impl Line {
    /// Constructor that mirrors the Python __`post_init`__ behavior:
    /// if `technology` is `None`, pick based on line type.
    #[must_use]
    pub fn new(
        line_type: LineType,
        technology: Option<Vec<String>>,
        lengths: Option<Vec<f64>>,
        fixed_pads_db: Option<Vec<f64>>,
        cable_db_keys: Option<HashMap<String, String>>,
    ) -> Self {
        let mut line = Line {
            line_type,
            technology: technology.unwrap_or_default(),
            lengths: lengths.unwrap_or_else(|| Line::default().lengths),
            fixed_pads_db,
            cable_db_keys: cable_db_keys.unwrap_or_else(|| Line::default().cable_db_keys),
        };

        // Emulate __post_init__
        if line.technology.is_empty() {
            line.technology = match line.line_type {
                LineType::Dc => vec![
                    "dc_NbTi".into(),
                    "dc_NbTi".into(),
                    "dc_NbTi".into(),
                    "dc_PhBr".into(),
                    "dc_PhBr".into(),
                ],
                _ => vec![
                    "dense".into(),
                    "dense".into(),
                    "dense".into(),
                    "coax".into(),
                    "coax".into(),
                ],
            };
        }

        line
    }

    /// Pure cable attenuation per meter (dB/m) for a cable technology.
    fn pure_cable_attenuation_per_m(tech: &str) -> f64 {
        match tech {
            "dense" => {
                // 1.5 dB/m
                1.5
            }
            "coax" => {
                // 3.2 dB/m at 4 K
                3.2
            }
            "dc_PhBr" | "dc_NbTi" => {
                // DC lines assumed to have negligible RF attenuation here.
                0.0
            }
            _ => panic!("Unsupported cable technology: {tech:?}"),
        }
    }

    /// Attenuation per stage [dB].
    #[must_use]
    pub fn attenuation(&self) -> Vec<f64> {
        // Base pads: override if provided, else defaults by type.
        let base: Vec<f64> = if let Some(ref pads) = self.fixed_pads_db {
            pads.clone()
        } else {
            match self.line_type {
                LineType::Drive => vec![26.0, 20.0, 10.0, 10.0, 0.0],
                LineType::Pump => vec![0.0, 15.0, 13.0, 15.0, 0.0],
                LineType::Ffl => vec![0.0, 0.0, 0.0, 10.0, 6.0],
                LineType::Dc => vec![0.0; 5],
            }
        };

        let cable_db: Vec<f64> = self
            .technology
            .iter()
            .zip(self.lengths.iter())
            .map(|(t, l)| Self::pure_cable_attenuation_per_m(t) * l)
            .collect();

        base.into_iter().zip(cable_db).map(|(b, c)| b + c).collect()
    }

    /// Cumulative attenuation up the stack [dB].
    #[must_use]
    pub fn cumulative_attenuation(&self) -> Vec<f64> {
        let att = self.attenuation();
        let mut result = Vec::with_capacity(att.len());
        let mut sum = 0.0;
        for a in att {
            sum += a;
            result.push(sum);
        }
        result
    }
}

/// Cryostat cabling model: stages, Carnot factors, and per-line data.
#[derive(Debug, Clone)]
pub struct Cabling {
    /// Stage names ordered cold → warm.
    pub stage_names: Vec<String>,
    /// Carnot factor per stage
    pub carnot_factors: Vec<f64>,
    /// Lines by logical type.
    pub lines: HashMap<LineType, Line>,
}

impl Default for Cabling {
    fn default() -> Self {
        // Python: ["MXC", "100mK", "Still", "4K", "50K"]
        let stage_names = vec![
            "MXC".to_string(),
            "100mK".to_string(),
            "Still".to_string(),
            "4K".to_string(),
            "50K".to_string(),
        ];

        let carnot_factors = STAGE_TEMPS
            .iter()
            .map(|&t| (T_EXT - t) / t)
            .collect::<Vec<f64>>();

        let mut lines = HashMap::new();
        lines.insert(
            LineType::Drive,
            Line::new(LineType::Drive, None, None, None, None),
        );
        lines.insert(
            LineType::Pump,
            Line::new(LineType::Pump, None, None, None, None),
        );
        lines.insert(
            LineType::Ffl,
            Line::new(LineType::Ffl, None, None, None, None),
        );
        lines.insert(
            LineType::Dc,
            Line::new(LineType::Dc, None, None, None, None),
        );

        Self {
            stage_names,
            carnot_factors,
            lines,
        }
    }
}

impl Cabling {
    /// Construct a `Cabling` object with optional overrides.
    #[must_use]
    pub fn new(
        stage_names: Option<Vec<String>>,
        carnot_factors: Option<Vec<f64>>,
        lines: Option<HashMap<LineType, Line>>,
    ) -> Self {
        let default = Cabling::default();

        Self {
            stage_names: stage_names.unwrap_or(default.stage_names),
            carnot_factors: carnot_factors.unwrap_or(default.carnot_factors),
            lines: lines.unwrap_or(default.lines),
        }
    }

    /// Linear attenuation factors per span, cold → warm.
    #[must_use]
    pub fn attenuation_factors(&self, line_type: LineType) -> Vec<f64> {
        let line = self
            .lines
            .get(&line_type)
            .unwrap_or_else(|| panic!("Unknown line type in lines map"));

        let a_cum = line.cumulative_attenuation(); // Vec<f64>
        let len = a_cum.len();

        let lin: Vec<f64> = a_cum.iter().map(|&a| db_to_val(a)).collect();

        let mut out = vec![0.0; len];
        if len > 0 {
            out[0] = lin[0];
        }
        for i in 1..len {
            out[i] = lin[i] - lin[i - 1];
        }

        out
    }

    /// Total 'macro' prefactor that weights chip power
    /// by the Carnot factors and per-stage linear attenuation contributions.
    #[must_use]
    pub fn m_prefactor(&self, line_type: LineType) -> f64 {
        let factors = self.attenuation_factors(line_type);

        self.carnot_factors
            .iter()
            .zip(factors.iter())
            .map(|(c, a)| c * a)
            .sum::<f64>()
    }
}

/// Type of interaction used when mapping `g` to `ε`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InteractionType {
    /// Stabilizer-type interaction.
    Stab,
    /// CNOT-type interaction.
    Cnot,
    /// Longitudinal interaction.
    Longitudinal,
}

// --------------------- Hardware struct ---------------------

/// Container for cat-qubit hardware parameters and derived coefficients.
#[derive(Debug, Clone)]
pub struct Hardware {
    /// Dissipation/linewidth (angular units) for the single-photon loss channel.
    pub k_1: f64,
    /// Dissipation/linewidth (angular units) for mode `b`.
    pub k_b: f64,
    /// Dephasing rate (angular units).
    pub k_phi: f64,
    /// External coupling rate (angular units).
    pub k_ext: f64,

    /// Inductive coupling (H).
    pub m: f64,
    /// Line impedance (Ohm).
    pub z: f64,
    /// Josephson energy [J].
    pub e_j: f64,
    /// Inductive energy [J].
    pub e_l: f64,
    /// Angular frequency of mode `a`.
    pub omega_a: f64,
    /// Angular frequency of mode `b`.
    pub omega_b: f64,
    /// Flux quantum [Wb].
    pub phi_0: f64,

    /// Participation / lever arm for mode `a`.
    pub vphi_a: f64,
    /// Participation / lever arm for mode `b`.
    pub vphi_b: f64,
    /// Participation / lever arm for the coupler.
    pub vphi_c: f64,
    /// Participation / lever arm for the target.
    pub vphi_t: f64,

    /// Thermal occupancy of mode `a`.
    pub ntha: f64,
    /// Thermal occupancy of mode `b`.
    pub nthb: f64,

    /// Measurement efficiency.
    pub eta: f64,

    /// Cryostat cabling model.
    pub cabling: Cabling,

    /// Coherent state amplitude α (inferred via `get_alpha`).
    pub alpha: f64,
}

impl Default for Hardware {
    fn default() -> Self {
        Self {
            // Dissipation/linewidths (angular units)
            k_1: 100.0,
            k_b: 24.0 * MHZ,
            k_phi: 0.08 * MHZ,
            k_ext: 400.0 * TWOPI, // 400 Hz in angular units (adjust if you have a Hz unit)

            // Circuit parameters
            m: 2.0 * PH,               // inductive coupling (H)
            z: 50.0,                   // Ohm
            e_j: 27.0 * GHZ * 1.0e-34, // J = Hz * hbar (J·s / rad)
            e_l: 40.0 * GHZ * 1.0e-34, // J = Hz * hbar (J·s / rad)
            omega_a: 4.29 * GHZ,
            omega_b: 7.21 * GHZ,
            phi_0: 2.0e-15, // Weber

            // Participation / lever arms
            vphi_a: 0.14,
            vphi_b: 0.19,
            vphi_c: 0.10,
            vphi_t: 0.20,

            // Thermal occupancies
            ntha: 0.02,
            nthb: 0.02,

            // Measurement efficiency
            eta: 0.4,

            // Cabling
            cabling: Cabling::default(),

            // alpha (will be inferred via get_alpha)
            alpha: 0.0,
        }
    }
}

impl Hardware {
    /// Construct a `Hardware` object, optionally overriding the cabling model.
    #[must_use]
    pub fn new(cabling: Option<Cabling>) -> Self {
        let mut hw = Hardware::default();
        if let Some(c) = cabling {
            hw.cabling = c;
        }
        hw
    }

    // --------------------- helpers / derived ---------------------

    /// This is a direct translation of the Python:
    ///   - same grid [0, 10] with 200 points,
    ///   - same `Gamma_bf` definition,
    ///   - same sign-change detection logic.
    #[allow(clippy::items_after_statements, clippy::cast_precision_loss)]
    pub fn get_alpha(&mut self, t_bf: f64) -> f64 {
        let gamma_bf = 1.0 / t_bf;

        // Gamma_bf(alpha) = gamma_bf - k_1 * alpha^2 * exp(-4 alpha^2)
        //                              - k_1 * ntha * exp(-2 alpha^2)
        let gamma_fn = |alpha: f64, this: &Hardware| -> f64 {
            gamma_bf
                - this.k_1 * alpha * alpha * (-4.0 * alpha * alpha).exp()
                - this.k_1 * this.ntha * (-2.0 * alpha * alpha).exp()
        };

        // Same grid as Python: np.linspace(0.0, 10.0, 200)
        let n_grid = 200usize;
        let a_min = 0.0;
        let a_max = 10.0;
        let step = (a_max - a_min) / (n_grid as f64 - 1.0);

        // Precompute grid and values like the Python code
        let mut grid = Vec::with_capacity(n_grid);
        let mut vals = Vec::with_capacity(n_grid);
        for i in 0..n_grid {
            let alpha = a_min + step * i as f64;
            grid.push(alpha);
            vals.push(gamma_fn(alpha, self));
        }

        // Helper: np.sign analogue
        let sign = |x: f64| -> i32 {
            if x > 0.0 {
                1
            } else if x < 0.0 {
                -1
            } else {
                0
            }
        };

        // Simple bisection solver (Python uses brentq; bisection is fine and
        // will give essentially the same root)
        fn bisect<F>(f: F, mut a: f64, mut b: f64, tol: f64, max_iter: usize) -> Option<f64>
        where
            F: Fn(f64) -> f64,
        {
            let mut fa = f(a);
            let fb = f(b);
            if fa == 0.0 {
                return Some(a);
            }
            if fb == 0.0 {
                return Some(b);
            }
            if fa * fb > 0.0 {
                return None;
            }

            for _ in 0..max_iter {
                let mid = f64::midpoint(a, b);
                let fm = f(mid);
                if fm == 0.0 || 0.5 * (b - a) < tol {
                    return Some(mid);
                }
                if fa * fm < 0.0 {
                    b = mid;
                } else {
                    a = mid;
                    fa = fm;
                }
            }
            Some(f64::midpoint(a, b))
        }

        let mut root = 0.0;

        // Mirror the Python loop:
        // for i in range(len(grid) - 1):
        //   if sgn[i] == 0: ...
        //   if sgn[i] + sgn[i+1] == 0: brentq(...)
        for i in 0..(n_grid - 1) {
            let v_i = vals[i];
            let s_i = sign(v_i);

            if s_i == 0 {
                root = grid[i];
                break;
            }

            let v_next = vals[i + 1];
            let s_next = sign(v_next);

            if s_i + s_next == 0 {
                // sign change -> bracket [grid[i], grid[i+1]]
                if let Some(cand) = bisect(|a| gamma_fn(a, self), grid[i], grid[i + 1], 1.0e-8, 100)
                    && cand.is_finite()
                {
                    root = cand;
                    break;
                }
            }
        }

        self.alpha = root;
        root
    }

    // --------------------- macro prefactors from cabling ---------------------

    /// Macro prefactor for pump lines (from cabling).
    #[must_use]
    pub fn mp(&self) -> f64 {
        self.cabling.m_prefactor(LineType::Pump)
    }

    /// Macro prefactor for drive lines (from cabling).
    #[must_use]
    pub fn md(&self) -> f64 {
        self.cabling.m_prefactor(LineType::Drive)
    }

    /// Macro prefactor used for Zeno-type operations (drive lines).
    #[must_use]
    pub fn mz(&self) -> f64 {
        self.cabling.m_prefactor(LineType::Drive)
    }

    /// Shared ATS-cancellation coefficient independent of the interaction type.
    #[must_use]
    pub fn generic_ats_cancellation_coef(&self) -> f64 {
        self.z
            * (self.phi_0 / (2.0 * PI * self.m)).powi(2)
            * (1.0 + 4.0 * self.e_j.powi(2) / self.e_l.powi(2))
    }

    /// Map from interaction type to the `g → ε` conversion prefactor.
    #[must_use]
    pub fn g_to_eps_coef(&self, interaction: InteractionType) -> f64 {
        match interaction {
            InteractionType::Cnot => HBAR / (self.e_j * self.vphi_c * self.vphi_t.powi(2)),
            InteractionType::Stab => 2.0 * HBAR / (self.e_j * self.vphi_b * self.vphi_a.powi(2)),
            InteractionType::Longitudinal => HBAR / (self.e_j * self.vphi_b * self.vphi_a.powi(2)),
        }
    }

    /// ATS cancellation coefficient for a given interaction type.
    #[must_use]
    pub fn ats_cancellation_coef(&self, interaction: InteractionType) -> f64 {
        let g = self.g_to_eps_coef(interaction);
        g * g * self.generic_ats_cancellation_coef()
    }

    /// ATS cancellation coefficient for stabilizer-type interactions.
    #[must_use]
    pub fn p(&self) -> f64 {
        self.ats_cancellation_coef(InteractionType::Stab)
    }

    /// ATS cancellation coefficient for CNOT-type interactions.
    #[must_use]
    pub fn c(&self) -> f64 {
        self.ats_cancellation_coef(InteractionType::Cnot)
    }

    /// ATS cancellation coefficient for longitudinal interactions.
    #[must_use]
    pub fn l(&self) -> f64 {
        self.ats_cancellation_coef(InteractionType::Longitudinal)
    }

    /// Dimensionless ratio `d = ℏ ω_b / κ_b`.
    #[must_use]
    pub fn d(&self) -> f64 {
        HBAR * self.omega_b / self.k_b
    }

    /// Dimensionless ratio `z = ℏ ω_a / κ_ext`.
    #[must_use]
    pub fn z_factor(&self) -> f64 {
        HBAR * self.omega_a / self.k_ext
    }

    /// Bit-flip time for a given α.
    #[must_use]
    pub fn t_bf_of_alpha(&self, alpha: f64) -> f64 {
        let rate = self.k_1 * alpha * alpha * (-4.0 * alpha * alpha).exp()
            + self.k_1 * self.ntha * (-2.0 * alpha * alpha).exp();
        1.0 / rate
    }

    /// Bit-flip probability during a gate of duration `T_gate`.
    #[must_use]
    pub fn p_z(&self, alpha: f64, t_gate: f64) -> f64 {
        alpha * alpha * self.k_1 * t_gate * (1.0 + 2.0 * self.ntha)
    }
}
