# Prism Overview

Prism is a Rust UI foundation that provides the **core layout, rendering, and event systems** used to build higher-level UI frameworks. Rather than acting as a full design system, Prism focuses on supplying the low-level primitives required to construct user interfaces in a structured and composable way.

**wgpu_prism** owns drawing instructions, shapes, colors, text layout, and SVGs. Prism uses
its CPU-only `drawable` feature and reexports those types through `prism::drawable`.
Prism owns UI layout, event dispatch, and the `Drawable` implementations that connect these
primitives to the UI tree. **Ramp** connects the Prism application to Maverick and enables
wgpu_prism's optional `renderer` feature for GPU rendering. Prism alone does not enable WGPU.

Prism is designed to serve as the base layer for higher-level frameworks such as **PTSD** and design systems like **Pelican UI**.

Drawing instructions flow through these layers; the primitives themselves belong to wgpu_prism:

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
wgpu_prism::Renderer (renderer feature)
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

Drawables describe renderable elements in the UI tree. They produce the shared instructions
defined by `wgpu_prism::drawable`, which `wgpu_prism::Renderer` consumes directly.

Components represent higher-level composable structures. A component may contain other drawables or components and defines how those children are laid out, rendered, and receive events.

This structure allows Prism to represent the UI as a hierarchical tree where complex components are composed from simpler visual elements.

The `Component` derive accepts two helper attributes: `#[skip]` excludes a field from drawable
children, and `#[events(...)]` lists event types for generic `OnEvent` implementations. Event
handling uses `#[on_event]` on typed implementations; asynchronous updates use `#[on_change]`.
The derive generates event dispatch and listener aggregation without override attributes.

### Layout

Drawables that need their children's completed geometry own that work in `Drawable::layout`.
Pelican's text editor positions its cursor there after text wrapping, and its growing grids
record the measured viewport for `OnChange`. The existing `Bin` container handles ordinary
child layout, drawing, and event dispatch.

`drawable::Text` fills its assigned width and wraps before measuring its height. Use
`Text::new(...).fit()` for a label that should use its natural width while still wrapping
when constrained. It exposes the renderer's text through `Deref` and `DerefMut`, including
spans, cursor state, and `cursor_rect()`. Cursor geometry uses the same wrapped lines as
rendering, including an empty final line after a newline. Width limits belong to the layout.

`drawable::Shape` also fills its assigned rectangle. It owns color, stroke, angle, and shape
variant; its parent layout supplies dimensions. `drawable::shape::Shape` and
`drawable::text::Text` are the rendering instruction data, not layout children.

Before drawing or dispatching input, Prism calls `request_width()`, selects the root width,
calls `request_height(width)`, selects the root height, then calls `layout(size)`. Both request
methods take `&mut self` and return `(minimum, maximum)`. Text wraps at its assigned width
before its height is requested; final placement uses the same width.

Layouts own their immediate children's measurements. The shared component implementation passes
child width requests to `Layout::request_width`, obtains their assigned widths with
`Layout::assign_width`, and passes their measured heights to `Layout::request_height`.
`Layout::layout(size)` uses those stored measurements to produce child areas. Each width request
replaces the previous measurements, including when children are inserted or removed. Descendant
measurements stay with descendant layouts; there is no separate range tree or settling loop.

### Images and icons

`hardware::Image`, `Audio`, `Video`, `Media`, and `Cursor` are the shared `media2` types,
imported as `media`. `Image` owns public `dimensions: (u32, u32)` and a shared pixel buffer.
`Image::new(dimensions, pixels)` takes generated pixels; `Image::read(reader)` decodes an encoded
image. Dereferencing exposes the pixel vector, and `image.as_ref()` borrows its RGBA8 bytes.
Clones share their pixels until a clone is mutated. Changing the display size reuses the GPU texture.
`image.write(writer, ImageFormat::Png)` encodes to any `Write` destination. `ImageFormat` supports
PNG, JPEG, WebP, GIF, BMP, ICO, TIFF, and QOI.

`drawable::Svg::read(reader)` validates SVG content and records its intrinsic dimensions. It is a
separate drawable, rasterized at the physical display size with a bounded shared cache.
`drawable::Icon(pub Svg, pub Color)` fills the SVG's alpha mask with the supplied color.

Raster and vector drawables accept the size assigned by their parent. Their instructions are
`Item::Image(shape, image, color)` and `Item::Svg(shape, svg, color)`. Playing video clones its
frame's `Image` into the same raster instruction, retaining the pixels without copying them.
The shape provides display dimensions, rotation, and a rectangular, rounded, or elliptical mask.
Its `color` field is unused; the instruction's `None` preserves the source colors, while
`Some(color)` replaces RGB and multiplies source alpha by color alpha.

`wgpu_prism` preserves the image's aspect ratio and crops centrally to cover the destination shape.
The instruction's bounds still clip the result. Display sizes use logical units; source image
dimensions remain pixel dimensions.

`Media` aggregates public `audio: Audio` and `video: Video` tracks. `Media::read(reader)` retains
an owned, seekable reader and decodes requested frames lazily. Cloning a track or Media creates
an independent edit branch that shares unchanged clips and cached frames. A `Cursor` attaches to
the original live timeline; cloned cursors share edits and caches but have independent positions.
Raw `AudioFrame` and `VideoFrame` values contain their timestamps, durations, and corruption flags.

`hardware.pick::<Visual>()` selects an image or a `Visual::Video(Media)` retaining both tracks.
`pick::<Video>()` selects only the video track; `pick::<Audio>()` selects only audio. Each also
supports `Vec<T>` for multiple selections. There is no content backend registration or media
conversion bridge.

Ramp's backend owns capture and picker services through `Hardware`. Camera, microphone, and
combined capture handles expose `next().await`, returning the next `VideoFrame`, `AudioFrame`, or
`MediaFrame`. Their timestamps remain relative to the capture source. Insert those frames into
a track or Media when recording; capture does not own or edit a media timeline.

`hardware.player(media.player())` attaches a core player to native audio output and scheduling.
The resulting `hardware::Player` starts paused. `set_mode(PlaybackMode)` selects `Default`,
`Live(Duration)`, or `Loop`: live follows incoming content at the requested delay, and loop
repeats the timeline. Call `start()` to play.

`frame()` returns the shared core `Frame`, including `video: Option<Arc<VideoFrame>>`, position,
optional duration, playing state, speed, and volume. Duration is unknown for live playback.
Loading a frame discards this handle's older unread updates; `next().await` returns the very next
update after that baseline. Each clone has an independent subscription to the same player.
Audio-only playback has `video: None`. `has_video()` reports track presence, so a gap between
video frames preserves the drawable layout. Playback modes belong to the core player; native
output errors belong to Ramp's shared wrapper.
Prism awaits `Player::next()` to schedule redraws and retains the frame Arc during rendering.

`media.write(writer, format)` directly writes a complete file to an owned seekable destination.
`media.encode(format)` produces successive encoded byte chunks for one continuous stream;
`Decoder` accepts those chunks. Format variants select container and codecs, including bitrate
when that encoder requires it. Compatible encoded packets are reused when possible.
Audio-only formats export only the audio track; absent audio produces an empty audio track.

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
component's registered `OnEvent<E>` handler and forwards unhandled events unchanged. Concrete handler
implementations receive this behavior automatically; no event-type list is needed on the component.

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
`#[on_event]`; omitting it produces a compile error saying to add `#[on_event]`.

Concrete handlers can live in separate modules or linked crates. This includes concrete
instantiations such as `MyComponent<u32>`; these do not require an event list either.
Refer to the component by its name rather than `Self` in the event type.

Concrete registration uses [inventory](https://docs.rs/inventory/latest/inventory/). Custom WebAssembly
embedders must follow its constructor-initialization requirements; see
[WebAssembly and constructors](https://docs.rs/inventory/latest/inventory/#webassembly-and-constructors).

Generic `#[on_event]` implementations require their event types in `#[events(...)]` after the
`Component` derive, above the component declaration:

```rust
use prism::drawable::{Component, Drawable, Layout};
use prism::events::{DynEvent, MouseEvent, OnEvent, on_event};
use prism::Context;

#[derive(Component, Clone, Debug)]
#[events(MouseEvent)]
pub struct GenericComponent<L: Layout + Clone + 'static, D: Drawable + Clone>(L, D);

#[on_event]
impl<L: Layout + Clone + 'static, D: Drawable + Clone> OnEvent<MouseEvent> for GenericComponent<L, D> {
    fn on(&mut self, _ctx: &mut Context, event: MouseEvent) -> Vec<Box<dyn DynEvent>> {
        vec![Box::new(event)]
    }
}
```

The derive calls these handlers directly, without inventory registration. Events outside the list
still use inventory, so concrete handlers can coexist with generic handlers for other event types.
Each listed handler must apply across the component's generic bounds. A missing entry produces a
compile error such as `add MouseEvent to #[events(...)] above GenericComponent`, even if the generic
handler is never instantiated. Keep generic implementations beside their component and use its
original type name so the attribute can access the derive's private registration check.

### Change notifications

`OnChange` awaits the future supplied by its attribute. Handles expose their own asynchronous
methods, such as `Instance::next()`, `Player::next()`, `Picked::next()`, and `Timer::next()`.
Dropping a pending wait must preserve unread updates and any scheduled deadline.
The source stays in its component; waiting does not clone it.

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

The attribute generates a private tag, an `OnChange<U, Tag>` implementation, and a future factory.
Concrete and generic implementations use the same generated dispatcher, without an inventory
registration or runtime registry lookup. Omitting the attribute produces a compile error saying
to add `#[on_change(source)]`.
The factory returns a callback that owns the typed update and invokes its handler; the update
needs no separate box or downcast.
Two handlers with the same update type can listen to different fields, such as `self.room.next()`
and `self.room2.next()`. The compiler
checks the awaited output against `U`; it does not automatically unwrap `Option` or `Result`.
Updates must be owned values that can outlive the source borrow.

Generic implementations remain separate from the component declaration and need no additional
field attribute:

```rust,ignore
#[on_change(self.timer.next())]
impl<D: Drawable + Clone> OnChange<()> for Dismissible<D> {
    fn on_change(&mut self, _ctx: &mut Context, _: ()) {}
}
```

The derive generates a private dispatcher and fallback trait. Each handler supplies a dispatcher
method; generic parameters are inferred from the component type. This selects the factory before
borrowing its source field. Unskipped fields without a handler use ordinary child listening.

Handlers must name the original struct type, not an alias, and be defined in the component's
module. Generic handlers must apply under the bounds declared by the component. Stronger bounds
cause a compilation error rather than silently disabling the listener. For behavior specific to
an instantiation such as `Dismissible<Text>`, put it inside a new component and implement that
component's handler. Each field supports one handler; duplicate source handlers fail to compile.

Each source expression can borrow one top-level field, including tuple fields. A field can have
one listener. The derive borrows those fields and child drawables independently,
and returns one `listen() -> ListenFuture<'_, Self>`. `ChangeHandler::select` races these futures using
`FuturesUnordered`, randomizing their initial order so continuously ready fields do not always
take priority. The result is an owned callback accepting the component and `&mut Context`.
Competing waits are dropped before the callback runs, releasing their source borrows.
The callback owns its update; updates do not need to implement `Clone`.
`Drawable::changed().await` returns the callback. `Prism::changed().await` invokes it with
the root drawable and `&mut Context`, then returns `()`. Inactive drawables stay pending.
Handlers run serially on the UI thread; no component needs a task context or a waker.

A drawable field with a change handler is watched by its owner instead of recursively waiting
on that field. This lets an owner observe a displayed `Player` once. Other children are
traversed automatically. Manual `Component` implementations return a `listen()` future;
manual `Drawable` implementations return a `changed()` future.
`ListenCallback<T>` is `Box<dyn FnOnce(&mut T, &mut Context)>`, and
`ListenFuture<'a, T>` is a boxed future returning that callback. Component callbacks use their
concrete type. Drawable callbacks use the default `dyn Any` target so the trait stays object safe;
the blanket implementation adapts between the two at that boundary.

`Drawables::listen_all()` gathers the listeners in a field. Its callbacks access the selected
element directly: vectors, slices, and arrays retain an index; maps retain a cloned key; tuples
and derived components access their fields. Optional values use their present child. Enum
callbacks access the variant that produced them. There is no separate update method or path
to search through the drawable tree. Invoke a callback on its original target before replacing
that target, removing its selected child, or changing its variant or vector order.

Enum components aggregate the active variant's children. Put variant-specific sources in child
components with their own `OnChange` implementations.
The context stays in Prism until a listener completes. Components do not need additional state
fields or shared mutable context.

Air `Instance<C>`, `Instances<C>`, and `Pending<R>` and hardware `Player` retain their native
`next()` futures. `Picked<T>::next()` returns this handle's selection once, then stays pending;
`get().await` and `try_get()` retain access to the cached result. Picker clones share the pending
operation and stored result, with one notification per handle. Dropping the final handle cancels
a pending selection.

Optional handles are handled explicitly in the listener expression:

```rust,ignore
#[on_change(async {
    match &mut self.picker {
        Some(picker) => picker.next().await,
        None => std::future::pending().await,
    }
})]
```

Collection owners select or sequence the futures they need using ordinary future combinators;
Prism does not impose collection-specific waiting behavior. An empty drawable tree stays pending.
Handles behind `Rc<RefCell<_>>` must be moved into their owner before waiting; Prism does not hold
runtime mutable borrows across awaits.

Initialize the view from its source's current state. For playback, `Player::frame()` establishes
the observation baseline before waiting for new frames. Timers are one-shot: `next().await`
returns `()` once and disarms the timer. Only an explicit `reset()` schedules another update;
waiting on a disarmed timer stays pending, and cancelling a wait does not restart its clock.

`Prism::on_event()` dispatches its event and drains the queue before returning, including events
emitted by handlers. Events are handled in queue order, without a batch limit or cycle detection.
Window resizing uses `on_event(WindowEvent::Resized(window))` through the same dispatch path.
Construction drains events emitted during application initialization. `Prism::changed().await`
waits for a source update, runs its callback, and drains any events it emits before returning.
`draw()` only measures and lays out the view and builds instructions; it does not dispatch events
or poll changes.
Maverick waits for native inputs, scheduled input futures, and application changes. Native
momentum and keyboard animation schedule work only while active.
Playback services still have their separate scheduling; this change does not remove those timers.

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

Prism is independent of the native window and GPU backend. Platform support is supplied by the
host application, such as Ramp with Maverick. The shared `wgpu_prism::drawable` types support
CPU layout and SVG rasterization without WGPU; its separate `renderer` feature supplies the
GPU backend. Ramp enables that feature when assembling the application.

## Examples

The Pelican UI checkout contains the maintained application demo for the Prism stack.

Frameworks built on Prism demonstrate how the core systems can be used to create structured UI environments. For example, **Pelican UI** builds on Prism to provide a full design system with predefined components and layouts.

Developers exploring Prism are encouraged to review these frameworks to understand how Prism's layout, rendering, and event systems can be composed into complete user interface frameworks.

## Discord

https://discord.gg/53ERRpS4S4

Join the Discord server to ask questions, discuss development, or share projects.
