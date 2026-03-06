# Compute Unit (CU) Notes

## Latest CU Snapshot (from `cargo test -- --no-capture`)

| Instruction | CU |
|-------------|-----|
| initialize  | 2015 |
| contribute  | 2654 |
| checker     | 8310 |
| refund      | 8919 |

The numbers come from the `compute_units_report` test in [programs/fundraiser/src/tests/mod.rs](../../programs/fundraiser/src/tests/mod.rs).

### Contribute path breakdown (init vs existing)

| Scenario               | CU  |
|------------------------|-----|
| contribute (init PDA)  | 2654 |
| contribute (existing)  | 1115 |

Measured via `compute_units_contribute_paths` in the same test module. The delta (~1.5k CU) is the cost of the contributor CreateAccount path.

## What drives CU per instruction

- initialize: PDA derivation/validation, minimal sysvars, no token CPI.
- contribute: Clock read, ATA validations, contributor PDA derive/validate, optional contributor CreateAccount CPI, state writes.
- checker: Token account + mint loads, PDA/ATA checks, TransferChecked CPI vault → maker ATA.
- refund: Clock read, PDA/ATA checks, contributor + fundraiser state borrows, TransferChecked CPI vault → contributor ATA, state rewrites.

## Ideas to reduce CU

1) Swap `TransferChecked` → `Transfer` in checker/refund when decimals are already trusted/validated; keep tests to guard correctness.
2) Avoid redundant borrows/derives: cache seed derivations once per instruction and minimize `try_borrow_*` calls.
3) Keep sysvar reads single-use: don’t re-fetch Clock/Rent in added code paths.
4) Separate contributor initialization from hot contribute path if contributors are reused; reduces CreateAccount CPI hits.

## On per-function CU granularity

Solana runtime reports CU per instruction, not per internal function. To approximate intra-instruction hotspots:
- Create micro-benchmark tests that isolate code paths (e.g., contribute with/without contributor init) and record CU.
- Temporarily stub out sections (e.g., skip validations) to see CU delta, then revert.
- Use `compute_units_report` as a template to add scenario-specific CU captures.# CreateAccount {}.invoke_signed()

- invoke_signed: 
    - requires proof of authority which checks the seed and programid
    - after proof checking, the PDA is marked as "is_signer = true"
- invoke():
    - signer of tx is marked as "is_signer = true" throughout the lifetime of tx.