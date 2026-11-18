# Integration Tests

End-to-end Rust pipeline tests live across workspace crates (for example `crates/harflow-cli/tests/cli_smoke.rs`) and verify `import -> analyze -> endpoints -> graph -> export -> validate -> report` against `fixtures/`.
