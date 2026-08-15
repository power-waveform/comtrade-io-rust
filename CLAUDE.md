# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

`comtrade-io` is a Rust library for parsing and exporting COMTRADE (IEEE C37.111) fault-recording files. It is a **rewrite of an existing Python library** (`comtrade-io`, baseline `develop` branch). Code comments and the docs in `docs/` frequently reference "对齐 Python" (align with Python) — this means matching the Python baseline's behavior. The design reports (`docs/rust重构软件设计报告.md`, `docs/rust重构需求分析报告.md`) are the authoritative spec.

Supported formats: `CFG`, `DAT`, `INF`, `DMF`, `CFF`, `DFR` (DFR read-only; HDR is a placeholder with no parse/write logic implemented).

## Commands

```bash
cargo build                       # build the library
cargo test                        # run all tests (integration + unit + doc-test)
cargo test --test cfg_tests       # run one integration test file (cfg/dat/inf/dmf/cff)
cargo test test_parse_binary_1999 # run a single test by name
cargo build --features gbk-builtin# build with the built-in GBK codec (see note below)
cargo fmt                         # format per rustfmt.toml (stable-only subset, see below)
```

## Formatting

`rustfmt.toml` targets the **stable** toolchain. Options that are nightly-only are present but
commented out and tagged `[nightly]` — on stable rustfmt ignores them *silently*, so leaving them
active made the config lie about what it does. Notably `imports_granularity`, `group_imports`,
`wrap_comments`, `comment_width`, and `normalize_comments` are NOT in effect: imports are only
sorted (`reorder_imports`), never merged or grouped, and comments are never rewrapped. Run
`cargo +nightly fmt` if you want those. Do not uncomment them for a stable run.

Note `cargo fmt` may need **two passes** to converge on this codebase (long `assert_eq!` macros in
`tests/`); `cargo fmt --check` must exit 0 before committing.

## Dependencies & Feature Flags

Minimal dependencies by design — no serde, chrono, or quick-xml. JSON/CSV/XML/time parsing are all hand-written.

- `encoding_rs` is the only runtime dependency, used for GBK codec.
- `gbk-builtin` feature (default **off**): swaps in a hand-written GBK codec instead of `encoding_rs`. **It is incomplete** — `gbk_to_unicode` returns `U+FFFD` for all CJK ranges, so it is only suitable for ASCII-heavy files. The default (encoding_rs) path is the working one.

## Architecture

### Parse / IO separation
Every format's core parser takes `&str` or `&[u8]` (e.g. `Config::from_str`, `DatFile::from_bytes`); the `from_file` methods are thin wrappers that read bytes and dispatch. This keeps parsers testable and embeddable. Follow this pattern for new code — do not entangle parsers with filesystem access.

### Top-level aggregate: `Comtrade` (`src/comtrade.rs`)
`Comtrade::from_path()` is the main entry point. It dispatches by file extension:
- `.cff` / `.dfr` → single-file mode (parse the composite file directly)
- anything else → multi-file mode: resolve sibling files by `parent + stem`, preserving the input's extension case (`.CFG` vs `.cfg`). `CFG` + `DAT` are required; `INF`/`DMF`/`HDR` are optional.

Equipment topology (`EquipmentGroup`) is built from DMF if present, else INF, else empty.

### Module dependency graph (acyclic by design)
```
comtrade ──> cfg / dat / hdr / inf / dmf / cff / dfr ──> equipment
   │            │     │      │      │     │     └──> encoding(cp1251), time, error
   └──> exporters ────┴──────┴──────┴─────┴──────────> encoding, time, error
```
The Python version had a circular import (`comtrade_file ↔ cfg`); the Rust version breaks it by putting file-group location in `comtrade.rs`. Do not add a back-edge from format modules to `comtrade`.

### DAT columnar storage
`DatFile` stores data column-major (`analogs: Vec<Vec<f64>>`, `statuses: Vec<Vec<u8>>`), mirroring the Python pandas DataFrame column-access pattern. `fit_to_config` truncates/pads columns to match `cfg.channels` counts and truncates rows to the last segment's `end_point`.

### Analog value scaling
On parse, raw values are converted to engineering values: `value = raw * multiplier + offset` (per `AnalogChannel`). On write, the inverse is computed. `ASCII` DAT applies a fixed 3-decimal precision on parse. Keep parse/write inverse-consistent or round-trip tests will break. Exception: `FLOAT32` stores engineering values directly per IEEE C37.111 — parse reads `f32` as the engineering value (skips mult/offset), write stores `f32` directly (no raw inversion).

### Encoding
- `CFG` / `INF` read GBK-first (`read_text_gbk`), falling back to UTF-8 if the GBK decode contains `U+FFFD`.
- `DMF` / JSON / CSV output is UTF-8.
- `DFR` WNDR text header is cp1251 (Cyrillic); DFR is read-only.
- `CFG` / `INF` write as GBK by default.

### Time (`src/time.rs`)
Custom `Timestamp` (no chrono), microsecond precision. `parse()` tries 15 format families in priority order (US `MM/DD/YYYY` first, to match `format_cfg` output). Non-leap-year `02/29` degrades to `02/28`.

### DMF XML
Hand-written lightweight XML pull parser (`xml_reader.rs`), namespace-prefix-agnostic. Unknown attributes/sections are preserved verbatim for round-trip fidelity.

## Key Conventions

- **Round-trip fidelity is a design goal**: every format should satisfy `parse → serialize → reparse` semantic equivalence. Unknown content (INF sections, XML attributes) must be preserved, not dropped. There are round-trip tests for each format — run them after touching any parser.
- **Unified `Error` enum** (`src/error.rs`): `Parse` carries `FileRole` + line number, `Binary` carries offset. Parsers must return `Result`, never `panic!`/`unwrap` on user data. Use the `Error::parse` / `Error::binary` / `Error::number` constructors.
- **`from_str` / `to_string` are inherent methods**, not trait impls — they carry `#[allow(clippy::should_implement_trait)]`. Do not "fix" them into trait impls.
- **Channel indexing gotcha**: CFG channel `index` fields are 1-based. `DatFile::analog_samples` takes a 0-based column index. `Comtrade::analog_channel(index)` takes a 1-based index and internally calls `analog_samples(index - 1)`.
- **Accessors carry no `get_` prefix** (Rust API guideline C-GETTER): `Config::analog`/`status`, `DatFile::analog_samples`/`status_samples`. Do not reintroduce `get_*` when adding accessors.
- **Out of scope** (per requirements doc, do not implement unless asked): HDR parsing, DFR writing, channel-recognizer heuristics, `CfgToEquipment` auto-generation.
- **`Segment` has two constructors, and three `Option` derived fields**: `samp_rate` / `end_point` come from the CFG file itself; `start_point` / `count` / `cycle_point_num` are all `Option` and are populated *only* by `recalculate_segments`, mirroring the Python baseline where all three are `| None` with `default=None`. Use `Segment::new(samp_rate, end_point)` when constructing from a file (CFG / DFR / INF paths all do this — derived fields stay `None`); use `Segment::derived(samp_rate, start_point, end_point, nominal_freq)` when recalculating. `derived` computes `count = end_point - start_point` (half-open `[start_point, end_point)`) and `cycle_point_num = samp_rate / nominal_freq`, defaulting to `DEFAULT_NOMINAL_FREQ` = 50Hz when the CFG frequency is `<= 0` or non-finite. `Sampling::to_cfg_text` writes only `samp_rate` / `end_point`, so CFG round-trip is unaffected by the derived fields. Note `nominal_freq` does **not** influence `samp_rate` itself — the sample rate comes purely from timestamp deltas.

## Tests

Integration tests live in `tests/` (one file per format) with fixtures in `tests/data/`. Fixtures are GBK-encoded where applicable; tests load them via `comtrade_io::encoding::decode_with(&bytes, Encoding::Gbk)` rather than `std::fs::read_to_string`. Inline `#[cfg(test)]` unit tests cover parsing edge cases. When adding a format behavior, add both an inline unit test (fast feedback) and a round-trip integration test.
