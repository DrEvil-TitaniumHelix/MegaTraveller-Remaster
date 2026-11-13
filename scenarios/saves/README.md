# Sample saves for the `[save]` scenarios (format 1.3)

Nothing here is committed except this file: a `.mtsav` holds the player's own T.DAT data segment and decoded pictures
(BYO law). The exe generates the sample from the before-save scenario with the player's files:

```
megatraveller --scenario scenarios/m7_starport_before_save.toml --scenario-check --save scenarios/saves/m7_sample.mtsav
```

`tests/scenario_loader.py` and the `cargo test` scenario suite regenerate it the same way (identical bytes: deterministic
headless clock and RNG). `scenarios/m7_after_restore.toml` restores it.
