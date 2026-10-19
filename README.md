# Prism Overview

Prism is a Rust UI foundation that provides the **core layout, rendering, and event systems** used to build higher-level UI frameworks. Rather than acting as a full design system, Prism focuses on supplying the low-level primitives required to construct user interfaces in a structured and composable way.

Prism owns the drawing instructions, geometry, colors, images, and text layout. **wgpu_prism** depends on Prism and renders those instructions on the GPU. **Ramp** connects the Prism application to Maverick and the renderer; Prism has no dependency on either backend.

Prism is designed to serve as the base layer for higher-level frameworks such as **PTSD** and design systems like **Pelican UI**.

Drawing instructions flow through these layers:

```text
Application
    ↓
Design System (e.g. Pelican UI)
    ↓
PTSD
    ↓
Prism
    ↓
Ramp
    ↓
wgpu_prism
```

## Layout System

Prism includes several layout primitives used to arrange components in a UI tree. These layouts define how child elements are positioned and sized relative to one another.

Primary layout primitives include:

* **Column** — vertically stacked children
* **Row** — horizontally arranged children
* **Stack** — layered elements positioned on top of each other
* **Wrap** — flowing layout that wraps children across lines

In addition to traditional layouts, Prism also provides display layouts designed to simplify conditional or structural UI patterns:

* **Enum** — renders variants of an enum-based UI state
* **Opt** — conditionally renders optional content
* **EitherOr** — switches between two alternative views
* **Bin** — wraps a single drawable object in a layout

These layouts allow UI frameworks built on Prism to construct complex interfaces while maintaining a consistent layout model.

## Components and Drawables

Prism separates visual elements into two primary categories:

* **Drawables**
* **Components**

Drawables describe the fundamental renderable elements in the system. Their Prism instructions can be consumed directly by `wgpu_prism`.

Components represent higher-level composable structures. A component may contain other drawables or components and defines how those children are laid out, rendered, and receive events.

This structure allows Prism to represent the UI as a hierarchical tree where complex components are composed from simpler visual elements.

### Images and icons

`hardware::Image` is shared pixel data. Its `Drawable` implementation accepts the size assigned by
its parent, allowing an image to shrink or enlarge inside a fixed-size layout. It emits a rectangle
at that size without copying or resizing the source pixels. `drawable::Icon(pub Image, pub Color)`
uses the same sizing and fills the image's alpha mask with the supplied color.

Image instructions are `Item::Image(shape, image, color)`. The shape provides display dimensions,
rotation, and a rectangular, rounded, or elliptical mask. Its `color` field is unused for images;
the instruction's `None` preserves the source colors, while `Some(color)` replaces RGB and multiplies
source alpha by color alpha. A fully transparent fill remains transparent.

`wgpu_prism` preserves the image's aspect ratio and crops centrally to cover the destination shape.
The instruction's bounds still clip the result. Display sizes use logical units; source image
dimensions remain pixel dimensions.

## Event System

Prism includes a flexible event system responsible for delivering input and lifecycle events throughout the UI tree.

Events are emitted continuously as the application runs. These can include events such as:

* mouse clicks
* pointer movement
* keyboard input
* frame updates (ticks)

Objects within the UI tree can listen for these events and react accordingly. Components can implement event handlers to process events and propagate them to their children when appropriate.

This event system forms the basis for all user interaction within frameworks built on Prism.

`Component::dispatch_event` receives a `Box<dyn DynEvent>`. Its default implementation dispatches to the
component's registered `OnEvent<E>` handler and forwards unhandled events unchanged. Components that
derive `Component` receive this behavior automatically; no event-type list is needed on the derive.

For an existing component `MyComponent`, a typed handler looks like this:

```rust
use prism::events::{DynEvent, MouseEvent, OnEvent, on_event};
use prism::Context;

#[on_event]
impl OnEvent<MouseEvent> for MyComponent {
    fn on(&mut self, _ctx: &mut Context, event: MouseEvent) -> Vec<Box<dyn DynEvent>> {
        vec![Box::new(event)]
    }
}
```

The handler owns its event; no `Clone` bound is required. Return an empty vector to suppress it, or
return replacement events to send those to the children. Each handler implementation must have
`#[on_event]`; omitting it produces a compile error identifying the missing registration.

Typed handlers can live in separate modules or linked crates. Each implementation must use concrete
component and event types, including concrete instantiations such as `MyComponent<u32>`; generic
`impl<T>` handlers are rejected. Refer to the component by its concrete name rather than `Self` in
the event type.

Registration uses [inventory](https://docs.rs/inventory/latest/inventory/). Custom WebAssembly
embedders must follow its constructor-initialization requirements; see
[WebAssembly and constructors](https://docs.rs/inventory/latest/inventory/#webassembly-and-constructors).

For generic components or custom boxed-event handling, add bare `#[on_dyn_event]` after the
`Component` derive and define an inherent `on_event` method. The derive's `dispatch_event` calls that
method in place of registry dispatch. No extra method name or event list is required:

```rust
use prism::drawable::{Component, Drawable, Layout};
use prism::events::DynEvent;
use prism::Context;

#[derive(Component, Clone, Debug)]
#[on_dyn_event]
pub struct GenericComponent<L: Layout + Clone + 'static, D: Drawable + Clone>(L, D);

impl<L: Layout + Clone + 'static, D: Drawable + Clone> GenericComponent<L, D> {
    fn on_event(&mut self, _ctx: &mut Context, event: Box<dyn DynEvent>) -> Vec<Box<dyn DynEvent>> {
        vec![event]
    }
}
```

## Emitters

Emitters translate raw input events into higher-level interaction events that are easier for UI systems to work with.

For example, a sequence of mouse events may be interpreted by an emitter and converted into a more meaningful UI event such as a button press.

Prism provides emitters for common interaction types, including:

* **Button**
* **Selectable**
* **TextInput**
* **NumericalInput**
* **Slider**
* **Scrollable**

These emitters allow UI frameworks to build interactive components without needing to process raw input events directly.

## Scope

Prism intentionally **does not provide design systems, visual styles, or application-level navigation**. These responsibilities belong to higher-level layers such as PTSD and design frameworks like Pelican UI.

## Platform Support

Prism is independent of the native window and GPU backend. Platform support is supplied by the host application, such as Ramp with Maverick, and its renderer. `wgpu_prism` provides the WGPU renderer for Prism instructions.

## Examples

Example applications demonstrating Prism are included in this repository. 

Frameworks built on Prism demonstrate how the core systems can be used to create structured UI environments. For example, **Pelican UI** builds on Prism to provide a full design system with predefined components and layouts.

Developers exploring Prism are encouraged to review these frameworks to understand how Prism's layout, rendering, and event systems can be composed into complete user interface frameworks.

## Discord

https://discord.gg/53ERRpS4S4

Join the Discord server to ask questions, discuss development, or share projects.
