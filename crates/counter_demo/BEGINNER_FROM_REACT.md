# GPUI for people who know TypeScript, React, and Hono

This doc assumes you are comfortable with **React** (components, state, JSX), **TypeScript** (types, `async`/`await`), and **Hono** (HTTP routes, handlers, middleware). None of that is “wrong” background—it is just **different** from how **Rust + GPUI** work.

The runnable example is still **`src/main.rs`** in this crate. Read this file with that code open side by side.

---

## The one-sentence map

| You know | In this demo |
|----------|----------------|
| React function component + `useState` | A **`struct`** (`CounterApp`) with fields + `impl Render` |
| `setCount(n)` / state updates | **Mutate** the struct (`this.count += 1`) then **`cx.notify()`** |
| JSX `<div onClick={...}>` | **`div().id(...).on_click(cx.listener(...))`** chained method calls |
| Virtual DOM diffing | GPUI rebuilds an **element description** when the view **notifies**; the framework handles the rest |
| Hono `app.get('/…', handler)` | **Not used here** — GPUI is a **desktop UI** framework, not an HTTP server. You would use **Axum**, **Hono on Cloudflare**, etc. for APIs separately |

---

## Rust in five minutes (if you know TypeScript)

**Types** — Often stricter than TypeScript at compile time. `i32` is a 32-bit integer; there is no “maybe a number” unless you use `Option<i32>`.

**No `null`** — Rust uses **`Option<T>`**: either `Some(value)` or `None`. You usually handle both with `match`, `if let`, or `?`.

**Errors** — Many fallible operations return **`Result<T, E>`** (`Ok` / `Err`). In app code in this repo you often see `.expect("…")` or `?` instead of try/catch.

**Ownership (just enough for UI)** — Values have one owner. References (`&`, `&mut`) borrow without taking ownership. Closures sometimes need `move` so they **own** captured variables; the compiler tells you when.

**Structs + `impl`** — A `struct` is like a typed object shape. `impl CounterApp { … }` adds methods. `impl Render for CounterApp` means “`CounterApp` can be drawn as UI,” similar to satisfying a TypeScript **interface**, but checked at compile time.

You do **not** need to master all of Rust before reading `main.rs`. Use errors as signposts: fix one at a time.

---

## React mental model vs GPUI

### Where state lives

**React:**

```tsx
const [count, setCount] = useState(0);
```

State is managed by React; you call `setCount` to schedule a re-render.

**GPUI (this crate):**

```rust
struct CounterApp {
    count: i32,
}
```

State is **ordinary fields** on a type. The framework holds that value inside an **`Entity<CounterApp>`** (created with `cx.new`). You change fields directly in event handlers.

### What triggers a re-render

**React:** Calling a state setter (or parent re-render) triggers reconciliation.

**GPUI:** After you change something that affects the UI, you call **`cx.notify()`** on the view’s context. That tells GPUI: “run `render` again for this entity.”

If you forget `cx.notify()`, the counter’s **logic** may change but the **screen** will not update.

### The UI description

**React:** You return JSX; it describes a tree.

**GPUI:** `render` returns something that implements **`IntoElement`** — usually a chain like:

```rust
div()
    .flex()
    .child(Label::new(format!("{}", self.count)))
```

Think of it as **JSX as fluent builder methods**, not XML-like syntax. Nesting uses **`.child(...)`** (and similar helpers) instead of children between tags.

### Event handlers and `this`

**React:** Handlers close over `count` / `setCount`, or you use a ref.

**GPUI:** **`cx.listener(|this, _event, _window, cx| { ... })`** wires the closure to the **current view**. Inside the closure, **`this` is `&mut CounterApp`**, so you can write `this.count += 1` like mutating a class instance, then **`cx.notify()`**.

The underscore parameters (`_event`, `_window`) mean “required by the API but unused in this snippet.”

### Clicks need an element identity

In GPUI, a plain `div()` is not enough for **`on_click`**. You first give the div a stable **`ElementId`** (here via **`.id(ElementId::Name("…".into()))`**). That turns it into a **stateful** element that can receive pointer events.

Rough React analogy: the framework needs a stable key or node identity for hit-testing and event routing—not the same as React `key` for lists, but the same *idea*: “this specific box is interactive.”

---

## Where Hono fits (and does not)

**Hono** describes **HTTP** behavior: routes, request/response, middleware, deployment on Workers, etc.

**GPUI** describes **native windows**: layout, input, painting, fonts. There is no `c.req` / `c.json()` here.

Typical split in your head:

- **Server / API** — TypeScript + Hono, or Rust + Axum, etc.
- **Desktop shell in this repo** — Rust + GPUI inside applications like Zed / Glass.

You *can* call HTTP from a GPUI app later (`reqwest`, etc.), but that is separate from how buttons and labels work.

---

## Read `main.rs` in order

1. **`CounterApp`** — Your component state (fields only).
2. **`impl Render for CounterApp`** — Your “JSX”: layout + labels + buttons.
3. **`fn main`** — Boot the OS app, load settings/theme/fonts (boilerplate shared with other binaries here), **`open_window`**, **`cx.new`** to create the root entity.

The **important learning loop** is inside `render`: **`on_click` → mutate `this` → `cx.notify()`**.

---

## Small glossary

| Term | Plain meaning |
|------|----------------|
| **`Entity<T>`** | A handle to app-owned state of type `T`; GPUI schedules rendering and events for it |
| **`Context<T>`** | While building or updating that entity, the callback context (access to `listener`, `notify`, spawn, etc.) |
| **`cx.new(|cx| …)`** | “Create a new entity; here is how to construct `T` given its first `Context`” |
| **`App` / “`cx` in `main`”** | The **application** context: open windows, globals, bindings, not tied to one view |

---

## When you feel stuck

- **“The UI doesn’t update”** — Did you call **`cx.notify()`** after changing state?
- **“`on_click` doesn’t exist on `div`”** — Did you add **`.id(...)`** before `on_click`?
- **“The borrow checker complains”** — Often you need a shorter block, a clone, or `move` on a closure; compare with examples in `sensor_dashboard` / `AGENTS.md`.

---

## After this file

- **`README.md`** (same folder) — Shorter reference, exercises, repo reading order.
- **`AGENTS.md`** (repo root) — Deeper GPUI patterns: `spawn`, `WeakEntity`, focus, actions.

You are not expected to memorize everything. Use this doc as a **translation layer**, then let the compiler guide the rest.
