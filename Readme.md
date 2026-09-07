[![ci](https://github.com/Alice-Bob-SW/qsharp-alice-bob-resource-estimator/actions/workflows/ci.yml/badge.svg)](https://github.com/Alice-Bob-SW/qsharp-alice-bob-resource-estimator/actions/workflows/ci.yml)

# Q# Resource Estimator for Alice & Bob's Architecture

This project estimates the amount of physical resources required to run quantum algorithms on [Alice & Bob](https://alice-bob.com)'s quantum architecture, which uses cat qubits and repetition code as described in [arXiv: 2302.06639](https://arxiv.org/abs/2302.06639) (LDPC codes might be added in the future).

The current version of the code specifically targets Shor's algorithm for solving the elliptic curve discrete logarithm problem and its subroutines. It improves the [python project](https://github.com/ElieGouzien/elliptic_log_cat) from which the results of the paper [Phys. Rev. Lett. 131, 040602](https://dx.doi.org/10.1103/PhysRevLett.131.040602) ([arXiv: 2302.06639](https://arxiv.org/abs/2302.06639)) originate.

The project consists of a Rust package containing a binary crate and a library crate as well as a Python API.

Big thanks to Mathias Soeken for the initial repository and for rebuilding the [Microsoft Q# resource estimator](https://github.com/microsoft/qdk/tree/main/source/resource_estimator) (in its version [1.6](https://github.com/microsoft/qdk/releases?page=3#release-v1.6.0) at the moment) to support our architecture.

## Installation

Two paths are possible.
### Cargo installation (Rust library and CLI only)

This is a standard Cargo crate.

Install Rust and the associated build toolchain for your operating system (Xcode Command Line Tools on macOS, build-essential on Linux, or Microsoft C++ Build Tools on Windows), see [Installation - The Rust Programming Language](https://doc.rust-lang.org/book/ch01-01-installation.html).
`cargo build --release` will then do its magic and build the estimator. The executable files are also available from the CI artifacts.

### Pixi installation (Rust library, CLI and Python API)

To run the setup commands, it is recommended to have Pixi installed on your machine, which can be done by following the installation guide [here](https://pixi.prefix.dev/latest/installation/).
Indeed, this repository includes a `pixi.toml` for reproducible environments, so that the command
```bash
pixi install
```
will create and manage the project environment for you, including installing Python, Rust, and any required dependencies defined in the project configuration. In addition, you’ll need a working native build toolchain for your operating system (Xcode Command Line Tools on macOS, build-essential on Linux, or Microsoft C++ Build Tools on Windows) in order to compile the Rust library behind both the CLI and the Python API, see [Installation - The Rust Programming Language](https://doc.rust-lang.org/book/ch01-01-installation.html).

In order to connect its Rust logic to Python, the project relies on [PyO3/maturin](https://github.com/pyo3/maturin). `pixi install` builds the python interface and installs it in the environment as an editable package, so no separate build step is needed. To rebuild after changing the Rust code, run `pixi install` again (or `pixi run maturin develop` for a quicker incremental build without going through the full dependency resolution).

## Program description

The program takes as input
- a quantum algorithm described via its "pre-layout" logical resource cost in the sense of [arXiv:2311.05801](https://arxiv.org/abs/2311.05801), i.e. `(qubits, cx, ccx)` where
	- `qubits` is the number of logical qubits,
	- `cx` and `ccx` are the numbers of expensive logical gates involved.
- an "error budget" in the sense of [arXiv:2311.05801](https://arxiv.org/abs/2311.05801), which represents the maximal logical error rates allowed for the target algorithm

Based on these, the purpose of the program is to predict the physical resource preparation conditions under which the target algorithm may eventually be executed on Alice & Bob’s proprietary architecture with an error rate consistent with the desired tolerance.

In practice, the machine’s execution costs, particularly energy costs, will depend heavily on the choice of easily adjustable machine parameters, such as the average number of photons per cat qubit, the distance of the repetition codes used to implement error correction, and the number of magic-state factories to provide. The program is precisely designed to calculate machine parameters that significantly reduce the combined product of the costs associated with parameter choices and the physical resource costs.

A physical resource cost, defined here as a number of physical qubits and a computation time on an Alice & Bob quantum machine, is computed at fixed parameters with Microsoft Azure Q# resource estimator.
To do so, it uses the logical-to-physical mapping described in:
- [arXiv:2311.05801](https://arxiv.org/abs/2311.05801) for the base Q# Resource Estimator model,
- [arXiv:2302.06639](https://arxiv.org/abs/2302.06639) for Alice & Bob architecture parameters regarding the cat qubit (average number of photons $\alpha^2$), the repetition code (code distance), and the different choices of magic state factories allowed,
- the function `logical_depth` from `src/counter.rs` for the logical_depths attributed to `cx` and `ccx` gates in the sense of [arXiv:2311.05801](https://arxiv.org/abs/2311.05801), i.e. how many logical cycles are necessary for their executions.

The program's output then consists of:
- Resources:
	- Total number of physical qubits
	- Runtime estimates
	- Number of magic state factories
	- Percentage of physical qubits involved in magic state production
- Physical parameters (data qubits and magic state factories are considered separately):
	- Code distance
	- Cat-qubit parameters (average photon number)
- Total error rate

The pre-layout logical resource cost input `(qubits, cx, ccx)` can be provided in three different ways:
1. **Qualtran Bloq** (only via the python interface): Qualtran calculates `(qubits, cx, ccx)`
2. **Q# program**: the interpreter extracts `(qubits, cx, ccx)`
3. **Explicit resources**: you pass `(qubits, cx, ccx)` directly

Note that **a few specific simplifying assumptions are performed** in the estimation of arbitrary Q# or Qualtran code. In particular, one-qubit gates are assumed to cost nothing compared to controlled gates and each CZ gate is exactly worth one CNOT gate, see [below](#supported-gates-and-their-costs) for more information.

## Usage

This mixed Rust/Python project is structured according to the recommendations from [Project Layout - Maturin User Guide](https://www.maturin.rs/project_layout.html).

### CLI Usage (Rust)
This Rust crate is designed as a library and also contains a standalone executable that estimates resources from either a Q# file or from the direct data of `(qubits, cx, ccx)`.
Use the subcommand `help` to have the documentation of the executable.

Basic form is either (depending if you are still developing or if you installed the executable):
* `cargo run -- [OPTIONS] <COMMAND>`
* `qsharp_alice_bob_resource_estimator_cli [OPTIONS] <COMMAND>`

**Commands:**
- `resources <qubits> <cx> <ccx>` — directly passes the pre-layout logical resource cost.
- `file <path-to-qsharp-file>` — reads a Q# file.

**Global options** (must appear before the command):
- `-f` or `--frontier` — prints a frontier of good parameter sets instead of a single estimate.
- `--json <file>` — also writes the computed estimate(s) as JSON to `<file>` (a single object, or an array when combined with `--frontier`).
- `--error-budget <topological> <magic> <rotation>` — detailed split into 3 components.
- `--error-total <value>` — overall error budget. Equivalent to using `--error-budget <value>/2 <value>/2 0`. Default value is `<value> = 0.333`.

**Important constraint:** you can use either `--error-total` or `--error-budget`, not both.

**Examples:**
- From explicit resources: `cargo run -- resources 40 10 10`
- Frontier mode with a file: `cargo run -- --frontier file qsharp/Adder.qs`
- Writing the estimate to a JSON file: `cargo run -- --json estimate.json resources 40 10 10`

Two example files can be executed:
- `cargo run --example=elliptic_log` uses as input the (pre-layout logical) resources required to run the elliptic curve discrete logarithm problem with bit size 256 and window size 18, as computed in [arXiv:2302.06639](https://arxiv.org/abs/2302.06639)
- `cargo run --example=from_qsharp` is equivalent to `cargo run -- --error-budget 0.0005 0.0005 0.0 file qsharp/Adder.qs`.

### Python Usage

In order to be able to use `import qsharp_alice_bob_resource_estimator` from python scripts in the repository, it is recommended to run them via
```bash
pixi run python script-name
```

For an end-to-end walkthrough, see the notebook `examples/getting_started.ipynb` via
```bash
pixi run jupyter notebook
```
(This should run on VSCode as well, but an issue has been reported on Windows platform where it was impossible to import `qsharp_alice_bob_resource_estimator` if the notebook was not run via the above command.)

## Supported gates and their costs

All cost assumptions made in this program have been designed with Shor's algorithm and its subroutines in mind. As such, they might not be relevant for the resource estimation of completely different algorithms.

### In Q# code

| Supported gates in Q# code                                      | cost in nb of cx | cost in nb of ccx |
| --------------------------------------------------------------- | ---------------- | ----------------- |
| `ccx`                                                           | 0                | 1                 |
| `cx`, `cy` and `cz`                                             | 1                | 0                 |
| `swap`                                                          | 3                | 0                 |
| supported one-qubit gates<br>(`x`, `y`, `z`, `h`, `sadj`, `s` ) | 0                | 0                 |
| measurements (`m`, `mresetz` & `reset`)                         | 0                | 0                 |

Using other gate types should result in a runtime error for being not implemented.

For further reading:
- language syntax: Q# reference ([Std.Intrinsic](https://learn.microsoft.com/en-us/qsharp/api/qsharp-lang/std.intrinsic/?source=recommendations), [Std.Canon | Microsoft Learn](https://learn.microsoft.com/en-us/qsharp/api/qsharp-lang/std.canon/), [Std.Measurement | Microsoft Learn](https://learn.microsoft.com/en-us/qsharp/api/qsharp-lang/std.measurement/))
- list of primitives supported by Q#: source code [qdk/source/compiler/qsc\_eval/src/backend.rs  · microsoft/qdk](https://github.com/microsoft/qdk/blob/571815d2ac9459c472a6c2b56de34eab5f22f08f/source/compiler/qsc_eval/src/backend.rs#L173)
- exact implementation of our rules: local file `counter.rs`

### In Qualtran code

| Supported gates in Qualtran code | cost in nb of cx | cost in nb of ccx |
| -------------------------------- | ---------------- | ----------------- |
| `CNOT`                           | 1                | 0                 |
| `Toffoli`                        | 0                | 1                 |
| `TwoBitCSwap`                    | 2                | 1                 |
| `C[CNOT]`                        | 0.5              | 0                 |
| `And`                            | 0                | 1                 |
| `And.adjoint()`                  | 0                | 0                 |
| any other qualtran gate          | 0                | 0                 |

No error should be returned due to an unsupported gate since gates cost nothing if they do not appear in the table.

For further reading:
- language syntax: [Qualtran documentation](https://qualtran.readthedocs.io/en/latest/bloqs/index.html#bloqs-library)
- exact implementation of our rules: local file `qualtran_interface.py`
- arguments behind the values in this table:
	- see [arXiv: 2302.06639](https://arxiv.org/abs/2302.06639)  appC.10 for the case of `TwoBitCSwap`
	- see [arXiv: 2302.06639](https://arxiv.org/abs/2302.06639)  app G.2 for the case of `And` gates with the application to adder circuits in mind
	- for `C[CNOT]`, we assume the controlled bit to take a "random", unbiased value in practical applications in cryptography, hence `C[CNOT]` should apply CNOTs half of the time in average

## Authors

- Mathias Soeken (initial version of the repository)
- Élie Gouzien
- Nicholas Gialouris
- Axel Pappalardo
