# Counter demo — learn Rust & GPUI in this repo

This crate is a **minimal GPUI app**: one integer, two buttons. Use it together with the source in `src/main.rs` to see how state, rendering, and input fit together in the Glass (Zed fork) codebase.

**Coming from TypeScript / React / Hono?** Start with **[`BEGINNER_FROM_REACT.md`](./BEGINNER_FROM_REACT.md)** — it translates concepts line-by-line before you deep-dive here.

## Run

From the repository root:

```sh
cargo run -p counter_demo
```

## What you are looking at (mental model)

1. **Single UI thread** — GPUI runs the window, layout, and your view logic on one thread. You do not “call React setState from another thread”; you mutate view state and **notify** the framework to redraw.

2. **`Entity<T>` and the root view** — `cx.new(|cx| CounterApp::new(cx))` creates an **entity**: a handle whose inner type is `CounterApp`. The window shows whatever that entity’s **`Render` implementation** returns.

3. **State lives on the struct** — `CounterApp { count: i32 }` is the model. No hooks: plain fields.

4. **`render` builds a tree** — `div()`, `.child(...)`, and style helpers describe layout and appearance (similar in spirit to utility-first styling in this codebase).

5. **Events mutate and notify** — In `on_click`, `cx.listener` gives you a closure where `this` is `&mut CounterApp`. After changing `this.count`, **`cx.notify()`** tells GPUI to run `render` again.

6. **Stable identity for clicks** — Interactive handlers like `on_mouse_down` / `on_click` are attached to a **stateful** div. In GPUI, call **`.id(ElementId::Name(...))`** on the `div()` first. Each button uses a distinct id.

7. **Hold to repeat (display refresh rate)** — **`on_mouse_down`** calls **`begin_hold`**: one immediate step, then **`Context::on_next_frame`** schedules a callback that (after **`HOLD_REPEAT_DELAY_MS`**) steps once per **frame** and chains another **`on_next_frame`**. That tracks **vsync**—about **120 FPS on a 120 Hz display**, instead of being capped by a ~30 ms background timer. **`hold_repeat_generation`** invalidates the chain on **`on_mouse_up`** / **`on_mouse_up_out`** so no stray callbacks keep firing. See also **`Window::request_animation_frame`** in GPUI, which schedules a redraw on the next animation frame.

That loop is the core: **state → render → user event → mutate state → notify → render**.

## Startup sequence (matches other binaries here)

In `main()`:

1. **`gpui_platform::application()`** — Builds the native application and event loop.
2. **`.with_assets(assets::Assets)`** — Asset source for fonts and bundled resources (same pattern as `sensor_dashboard`).
3. **`.run(|cx| { ... })`** — App initialization; `cx` is the **App** context.
4. **`SettingsStore`** + **`theme::init`** — The shared theme system expects default settings to exist (same idea as `sensor_dashboard` / Zed UI).
5. **`assets::Assets.load_fonts`** — Loads fonts so `Label` and text render correctly.
6. **`cx.open_window(..., |_, cx| cx.new(...))`** — Opens a window; the closure creates the **root view** entity.

If you strip (4)–(5), text and theming may break; this demo keeps the same baseline as the rest of the workspace.

## Reading order in this repository

1. **`crates/counter_demo/src/main.rs`** — End-to-end: smallest possible `Render` + click handlers.
2. **`crates/sensor_dashboard/src/main.rs`** — Same boot pattern, plus `cx.spawn` and `WeakEntity` for a background loop (not needed for a counter, but the next step when you add timers or async work).
3. **`crates/sensor_dashboard/src/render.rs`** — More `on_click` / `cx.listener` / `cx.notify` examples at scale.
4. **`AGENTS.md`** (repository root) — GPUI concepts: `Entity`, `Context`, `spawn`, `update`, etc.

## Exercises (learning by doing)

Do these in order; each one teaches one idea.

1. **Reset** — Add a third control that sets `count` to `0` and calls `cx.notify()`.
2. **Keyboard** — Add `on_key_down` on the root `div` (see `sensor_dashboard` for patterns) to increment on `=` and decrement on `-`.
3. **Bounds** — Store `WindowBounds` in a `const` or helper so you can experiment with window size in one place.
4. **Extract a subview** — Move the two buttons into a small helper that returns `impl IntoElement`, or into another type that implements `Render`, to practice composition.

## Rust concepts this file touches

| Concept | Where |
|--------|--------|
| Struct as UI model | `CounterApp { count }` |
| Trait implementation | `impl Render for CounterApp` |
| Mutable access in closures | `cx.listener(\|this, _, _, cx\| { ... })` |
| Saturating arithmetic | `saturating_add` / `saturating_sub` avoid overflow panics in debug builds |

## Related crate

- **`sensor_dashboard`** — Same stack (GPUI, theme, assets, settings), larger UI to explore after this demo.
