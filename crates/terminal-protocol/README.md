# terminal-protocol

Structured protocol and replay types for Hera terminal streams.

This crate defines parser-facing events and payload models that Hera can expose
without leaking parser implementation details such as `vte::Perform`.
