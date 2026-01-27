# ADR-002: Config File Format

## Status
Accepted

## Context
The application needs a configuration file to store user preferences such as:
- Default Whisper model
- Language for transcription
- History limits (max entries, max age)
- Auto-copy setting
- Global hotkey configuration (future)

The configuration should be:
- Human-readable and editable
- Easy to parse programmatically
- Support common data types (strings, numbers, booleans, optional values)
- Have a clear structure for nested/hierarchical data if needed

Common options considered:
- TOML (Tom's Obvious Minimal Language)
- JSON
- YAML
- INI-style
- Custom format

## Decision
**Use TOML format** for the configuration file (`config.toml`).

The application already uses TOML via the `toml` crate (version 0.8) as seen in [Cargo.toml](Cargo.toml) and [src/config.rs](src/config.rs).

## Rationale

### Why TOML
1. **Human-readable**: TOML is designed to be easily readable and writable by humans
2. **Rust ecosystem**: Excellent support via `toml` crate with `serde` integration
3. **Type safety**: Strong typing with serde derive macros
4. **Comments**: Supports comments, making config files self-documenting
5. **Common in Rust**: TOML is widely used in Rust projects (Cargo.toml, etc.)
6. **Simple structure**: Flat config structure doesn't require complex nesting

### Why not JSON
- No comments support
- Less human-friendly (requires quotes, commas, etc.)
- More verbose for simple key-value pairs

### Why not YAML
- More complex parsing
- Indentation-sensitive (error-prone)
- Larger dependency footprint
- Less common in Rust ecosystem

### Why not INI
- Less structured
- Limited type support
- No standard for optional/nullable values

## Consequences

### Positive
- ✅ Easy for users to edit manually if needed
- ✅ Supports comments for documentation
- ✅ Strong Rust ecosystem support
- ✅ Type-safe parsing with serde
- ✅ Consistent with Rust tooling (Cargo.toml)
- ✅ Simple, flat structure fits current needs

### Negative
- ⚠️ Less common than JSON (but still widely used)
- ⚠️ Requires `toml` crate dependency (already present)
- ⚠️ Not as universal as JSON for cross-language tools

## Implementation
Current implementation in [src/config.rs](src/config.rs):
- Uses `serde` with `Serialize` and `Deserialize` traits
- Uses `toml` crate for parsing and serialization
- Config file location: `~/.config/voice-dictation/config.toml`
- Default values provided via `Default` trait implementation

Example config.toml:
```toml
default_model = "ggml-base.bin"
language = "uk"
history_max_entries = 500
history_max_age_days = 90
auto_copy = false
# hotkey = "Ctrl+Shift+D"  # Future feature
```

## Related Files
- [src/config.rs](../../src/config.rs) - Config struct and TOML serialization
- [Cargo.toml](../../Cargo.toml) - Dependencies including `toml = "0.8"`
