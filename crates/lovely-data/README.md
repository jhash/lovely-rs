# lovely-data

Reserved binary for a future split where per-app SQLite files live on a separate VM from the web service. v1 ships a stub `main.rs` that exits non-zero with an explanatory message.

When implemented, this binary will host the `RemoteSqliteAppStore` impl (RPC server) and the web service will swap in a `RemoteSqliteAppStore` client behind the same `SqliteAppStore` trait — no changes to handlers required.
