# Feature crate boundaries

- Each crate under `crates/` implements one named feature. Do not group unrelated features into catch-all packages such as `core`, `client`, `common`, `services`, or `utils`.
- Keep a feature's data, rules, adapters, and tests together. For example, remote repository HTTP requests and their saved credentials belong to the remote repository feature; an adapter does not require its own crate just because it uses a different technology.
- Executable startup and composition belong under `apps/`. Feature libraries must not depend on eframe, native widgets, or a desktop event loop. Pass callbacks or plain data across those boundaries.
- Keep workspace dependencies acyclic and explicit. Repository observation is independent of authored plans; the plan feature consumes repository observations when importing a group. Session persistence depends on the plan format. Layout comparison consumes repository and layout features.
- Use modules to split responsibilities within a feature rather than creating a crate for every source file.
