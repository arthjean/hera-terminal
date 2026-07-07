# terminal-pty

PTY runtime boundary for Hera terminal sessions.

This crate owns process IO, resize, lifecycle and platform transport details.
It keeps PTY implementation types out of `terminal-core` and exposes Hera-owned
runtime abstractions.
