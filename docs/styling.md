# Styling Notes

Glory keeps styling in the same builder-pattern surface as the rest of the UI.
Plain classes are still the default:

```rust
div().class("todoapp").show_in(ctx);
```

For component-local CSS, use `scoped_css`:

```rust
let scope = glory_core::web::scoped_css(
    r#"
    :scope {
        display: grid;
        gap: 0.5rem;
    }

    button {
        color: red;
    }

    :global(body) {
        margin: 0;
    }
    "#,
);

style().text(scope.css().to_owned()).show_in(ctx);
div().class(scope).fill(button().text("Save")).show_in(ctx);
```

`ScopedStyle` generates a stable `gly-scope-...` class from the CSS text and
rewrites normal selectors under that class. Use `:scope` when the rule should
target the scoped element itself, and `:global(...)` for an intentionally
unscoped selector.

Supported today:

- regular selector lists, for example `button, a:hover`
- `:scope`
- top-level `:global(...)`
- nested `@media`, `@supports`, and `@container` blocks

At-rules such as `@keyframes` and `@font-face` are passed through unchanged.
This is scoped-style support, not a full CSS Modules implementation; class name
imports and build-time CSS file transforms remain CLI work.
