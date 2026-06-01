# Desktop Renderer Follow-up Tasks

This file splits IMP-009 into concrete implementation tasks.

- [x] Replace the placeholder Wry interpreter with a DOM command interpreter.
- [x] Consume create, attribute, property, class, text, HTML, insert, remove, and event attachment commands.
- [x] Forward browser-side events through a stable IPC payload shape.
- [x] Add Rust unit coverage for the full renderer command surface.
- [x] Add coverage that the embedded JS interpreter exposes all renderer command handlers.
