# Vide Language Server Architecture

Vide keeps LSP protocol mechanics in `crates/vide-lsp-runtime`. The runtime owns typed request
and notification routing, request lifecycle and cancellation, server-to-client requests, response
encoding, retries, and accepted-response effects.

`GlobalState` contains Vide's mutable language state. Read-only requests run against a
`GlobalStateSnapshot`; notifications and explicitly mutable requests run on the main loop. VFS and
background compiler events remain domain events handled by the main loop rather than protocol
concerns.

Native stdio and browser/WASM are transport adapters over the same runtime router. The native
adapter blocks on stdio and internal channels. The browser adapter pushes JSON messages into the
same router and drains outgoing messages through its FFI polling surface. Neither adapter contains
request handlers or a second dispatch implementation.
