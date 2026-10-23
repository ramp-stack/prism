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

`Context::background` supplies a full-window color behind the UI, including the regions outside
the safe area. It defaults to transparent and can be changed by the application or design system.
Prism draws the background once before content; its alpha is preserved. Content layout, clipping,
and pointer events remain confined to `Window::area()`, which applies the safe-area insets.
Resizing changes the background dimensions without resetting its color.

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

### Caret layout

`Drawable::caret(&LayoutTree)` returns `Option<Caret>` for an active editor. `Caret` contains a
stable editor ID and a local rectangle whose height is the edited line's height. Return `None`
when the editor is inactive. `drawable::text::Text::cursor_rect()` measures that rectangle using
the same wrapped lines as text rendering, including an empty final line after a newline.

Components find a descendant caret and translate it through their laid-out child offsets by
default. A custom `Drawable` wrapper must forward `caret` just as it forwards layout and events.
Hidden children excluded from `children()` do not participate.

Derived components can override this query with `#[caret]` and an inherent
`fn caret(&self, layout: &LayoutTree) -> Option<Caret>`. Add `#[after_layout]` and an inherent
`fn after_layout(&mut self, layout: &mut LayoutTree)` to adjust child offsets after the children
have laid out. Hooks must preserve the child order and tree structure. Pelican uses this hook
to position the visible caret after text wrapping.

Before drawing or dispatching input, Prism settles changed size ranges with up to four extra
layout passes. Layout hooks must converge: changes to text width should
produce stable size ranges, not toggle between requests.

### Images and icons

`hardware::Image` is a shared raster content handle supplied by the backend.
`hardware.read_image(reader)` reads encoded content and its intrinsic dimensions without decoding
all pixels. `image.dimensions()` returns its intrinsic `(width, height)`.
`Image` implements `Deref<Target = Arc<RgbaImage>>`; the first pixel access decodes lazily.
The backend retains the shared pixels, using the existing corrupted-image artwork if decoding fails.
`hardware.image_from_rgba(pixels)` accepts generated pixels directly. Clones share the content;
changing its display size reuses the same decoded pixels and GPU texture.

`drawable::Svg::read(reader)` validates SVG content and records its intrinsic dimensions. It is a
separate drawable, rasterized at the physical display size with a bounded shared cache.
`drawable::Icon(pub Svg, pub Color)` fills the SVG's alpha mask with the supplied color.

Both drawables accept the size assigned by their parent. Their instructions are
`Item::Image(shape, image, color)` and `Item::Svg(shape, svg, color)`. The shape provides display
dimensions, rotation, and a rectangular, rounded, or elliptical mask. Its `color` field is unused;
the instruction's `None` preserves the source colors, while `Some(color)` replaces RGB and multiplies
source alpha by color alpha. A fully transparent fill remains transparent.

`wgpu_prism` preserves the image's aspect ratio and crops centrally to cover the destination shape.
The instruction's bounds still clip the result. Display sizes use logical units; source image
dimensions remain pixel dimensions.

`hardware::Media` represents audio, video, or both. `hardware.read_media(reader)` imports a
complete file, including animated GIFs. `copy(range, Tracks::Audio)` selects an independent
audio timeline; `Tracks::Video` and `Tracks::Both` select the other combinations.
`replace(range, &replacement, tracks)` edits those tracks, and `append(&media)` adds a segment.
Clones share edits. Use the hardware factories to create an empty timeline, samples, or an image
with an explicit duration.

`hardware.pick::<Visual>()` selects an image or a video. `pick::<Video>()` selects video while
preserving its audio track; `pick::<Audio>()` selects an audio-only asset. Each also supports
`Vec<T>` for multiple selections. `Video(pub Media)` and `Audio(pub Media)` expose the selected
timeline through `.0` and implement `AsRef<Media>` for playback. They add no processing methods.

Ramp's backend owns capture and picker services through `Hardware`. Keep hardware alive while
recording or awaiting a selection. Dropping its last clone stops capture and cancels pending
selections; operation handles retain recorded media and completed results.

`hardware::Player::new(impl AsRef<Media>)` accepts media or a capture handle and starts paused.
Retain capture separately to keep recording. `set_mode(PlaybackMode)` selects `Default`,
`Live(Duration)`, or `Loop`: default waits at the available end, live follows incoming content
at the requested delay, and loop repeats the timeline. Call `start()` to play.

`frame()` returns the current `Frame`, including an optional image, position, duration, playing
state, speed, volume, mode, and any playback error. It also discards this handle's unread updates.
`next().await` returns the very next queued frame after that baseline. Each clone has an independent
subscription. Audio-only media uses the same interface with `image: None`; control updates retain
the current image when video is present.

Ramp converts Maverick's frame values at the backend boundary. Prism forwards those snapshots
through `ChangeSource` and `OnChange` without constructing a second playback state or timer.
Drawing reads `frame()`; layout reads the content dimensions without consuming an update.

`media.write(writer, Format::Mp4_Mpeg4p2_Aac)` explicitly selects the container and codecs.
`Format::extension()` returns the matching suffix. Audio-only formats extract the audio track;
if there are no audio samples, the output contains an empty audio track.

## Event System

Prism includes a flexible event system responsible for delivering input and lifecycle events throughout the UI tree.

Events are emitted in response to input and window changes, including:

* mouse clicks
* pointer movement
* keyboard input
* window resizing and focus changes
* native keyboard visibility changes

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

### Change notifications

A `ChangeSource` exposes `next().await`, which returns `Some(update)` when data changes and
`None` when the source is finished. Dropping a pending wait must preserve unread updates and
any scheduled deadline. The source stays in its component; waiting does not clone it.

Put `#[on_change(source_expression)]` on each handler implementation. The expression returns a
cancelable future whose output must match the handler's update type. Use `#[skip]` on source
fields that are not drawn:

```rust,ignore
use prism::air::{Instance, Update};
use prism::drawable::Component;
use prism::events::{on_change, OnChange};

#[derive(Clone, Debug, Component)]
struct Chat {
    layout: ChatLayout,
    messages: MessageList,
    #[skip]
    room: Instance<Room>,
}

#[on_change(self.room.next())]
impl OnChange<Update<RoomResult>> for Chat {
    fn on_change(&mut self, ctx: &mut prism::Context, update: Update<RoomResult>) {
        self.refresh(ctx, update);
    }
}
```

The attribute generates a private tag and an `OnChange<U, Tag>` implementation, then registers
the component, source field, and future factory. Two handlers with the same update type can
listen to different fields, such as `self.room.next()` and `self.room2.next()`. The compiler
checks the awaited output against `U`; it does not automatically unwrap `Option` or `Result`.
Updates must be owned values that can outlive the source borrow. Registered implementations
require concrete component and update types.

Each source expression can borrow one top-level field, including tuple fields. A field can have
one registered listener. The derive borrows those fields and child drawables independently,
and returns one `changed() -> ChangeFuture<'_>`. `Change::select` races these futures using
`FuturesUnordered`, randomizing their initial order so continuously ready fields do not always
take priority. The completed future returns a deferred `Change`; Prism drops competing waits
before applying the handler with mutable access to the component and `Context`. Callbacks run
serially on the UI thread. No component needs a task context or a waker.

A registered drawable field is watched by its owner's handler instead of recursively waiting
on that field. This lets an owner observe a displayed `Player` once. Other children are
traversed automatically. Manual `Component` and `Drawable` implementations also return a single
`changed()` future. `Change::child` and `Change::map` route its result through the same container;
apply it before changing that container. For custom aggregation, put `#[changed]` on a component
and implement its own `fn changed(&mut self) -> ChangeFuture<'_>`. That method owns all listening,
while the derive still supplies child and layout methods. Enum components aggregate their
children; variant-specific sources need custom aggregation.

Built-in sources include Air `Instance<C>`, `Instances<C>`, and `Pending<R>`, hardware `Picked<T>`,
`Player` and `Timer`. `Option<S>` is inactive while `None`; `BTreeMap<K, S>` waits
concurrently and returns a batch of ready `(key, update)` values; `VecDeque<S>` consumes sources
in order. Sources behind `Rc<RefCell<_>>` must be moved into their owner before this migration;
Prism does not hold runtime mutable borrows across awaits.

For these optional or finite sources, use
`#[on_change(ChangeSource::changed(&mut self.source))]` and import `ChangeSource`. This adapter
returns the update itself and stays pending if `next()` returns `None`, avoiding repeated
notifications from an inactive source. An empty drawable tree also stays pending.

Initialize the view from its source's current state. For playback, `Player::frame()` establishes
the observation baseline before waiting for new frames. Pickers deliver their result once through `OnChange`;
`get().await` and `try_get()` retain access to the cached result. Picker clones share the pending
operation and stored result, with one change notification per handle. Dropping the final handle
cancels a pending selection. Timers are one-shot: only an
explicit `reset()` schedules another update, and cancelling a wait does not restart its clock.

`Prism::changed().await` waits for a source update or already queued events. `draw()` drains
queued events and immediately ready changes, settles layout, and builds instructions without
broadcasting a frame event. Maverick waits for native inputs, scheduled input futures, and
application changes. Native momentum and keyboard animation schedule work only while active.
Playback services still have their separate scheduling; this change does not remove those timers.

Field-level `#[on_change]`, `OnChange<Source>`, and `changes() -> Vec<ChangeFuture<'_>>` are
replaced by the implementation attribute, tagged update handlers, and single `changed()` future.


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

The Pelican UI checkout contains the maintained application demo for the Prism stack.

Frameworks built on Prism demonstrate how the core systems can be used to create structured UI environments. For example, **Pelican UI** builds on Prism to provide a full design system with predefined components and layouts.

Developers exploring Prism are encouraged to review these frameworks to understand how Prism's layout, rendering, and event systems can be composed into complete user interface frameworks.

## Discord

https://discord.gg/53ERRpS4S4

Join the Discord server to ask questions, discuss development, or share projects.
