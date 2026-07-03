# Pre-push checklist

Before any push, run both:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude bl4-save-editor -- -D warnings
```

Both must pass with zero output / zero errors before pushing.
