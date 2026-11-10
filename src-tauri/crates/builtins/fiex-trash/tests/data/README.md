# Finder `.DS_Store` fixture

The bounded reader in `src/macos_ds_store.rs` is adapted from
`trash-core::macos` in `SecurityRonin/trash-forensic` and modified by Fiex to
look up one Trash entry. It is licensed under Apache-2.0; the license text is
included as `LICENSE-APACHE`.

`putback_types.DS_Store` is the regression fixture from
`SecurityRonin/trash-forensic`'s `core/tests/data/putback_types.DS_Store`. It is
licensed under Apache-2.0; the license text is included as `LICENSE-APACHE`.
The fixture contains Finder put-back records mixed with unrelated typed records
so the parser test covers the failure mode where an unknown record key used to
abort parsing the entire file.
