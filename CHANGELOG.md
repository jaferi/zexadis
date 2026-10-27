# Changelog

Notable changes to Zexadis are documented here.

## [0.5.1] - 2026-10-27

### Added
- Sharded in-memory storage engine.
- Binary keys and values.
- TTL-based expiration with lazy removal.
- GET, SET, DEL, and EXISTS commands.
- Binary wire protocol V1 with frame encoding, decoding, and validation.
- TCP server for client access.
- Initial protocol and TCP end-to-end tests.

### Notes
- CAS and batch execution are not yet implemented.
- APIs and protocol behavior may evolve during Alpha.
- Production readiness has not been established.
