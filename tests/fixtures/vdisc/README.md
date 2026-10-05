# VDISC contract fixtures

`transport-two-track.vdisc` is a tiny two-track silent WAV fixture for physical
transport integration tests. Generate it independently (without rewriting the
contract fixtures) with `rtk cargo run -p vdisc-core --example generate_transport_fixture`.

Generated from synthetic 10 ms, mono, 48 kHz, signed 16-bit PCM silence.

| File | Expected result |
| --- | --- |
| valid-v1.vdisc | Accept classic Stored ZIP |
| valid-v1-zip64.vdisc | Accept ZIP64 Stored ZIP with 64-bit data descriptors |
| corrupt-payload.vdisc | Reject stale payload SHA-256, even with a correct ZIP CRC |
| missing-manifest.vdisc | Reject missing required manifest |
| future-version.vdisc | Reject unsupported format version |
| duplicate-manifest.vdisc | Reject duplicate entry |
| path-traversal.vdisc | Reject noncanonical traversal path |
| malformed-manifest.vdisc | Reject invalid JSON |

Regenerate from the repository root:

```bash
cargo run -p vdisc-core --example generate_v1_fixtures
```

The fixture writer lives in test support and is not the Objective 13 burner.
