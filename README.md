# Prism Overview

Prism is a Rust UI foundation that provides the **core layout, rendering, and event systems** used to build higher-level UI frameworks. Rather than acting as a full design system, Prism focuses on supplying the low-level primitives required to construct user interfaces in a structured and composable way.

Prism interacts directly with **wgpu_canvas**, which is responsible for GPU rendering. Through this integration, Prism provides the infrastructure necessary for drawing text, shapes, and images while managing layout and event propagation across the UI tree.

Prism is designed to serve as the base layer for higher-level frameworks such as **PTSD** and design systems like **Pelican UI**.

Together these layers form the following architecture:

```text
Application
    ↓
Design System (e.g. Pelican UI)
    ↓
PTSD
    ↓
Prism
    ↓
wgpu_canvas
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

Drawables represent the fundamental renderable elements in the system. These include primitives such as text, shapes, and images that are ultimately drawn through `wgpu_canvas`.

Components represent higher-level composable structures. A component may contain other drawables or components and defines how those children are laid out, rendered, and receive events.

This structure allows Prism to represent the UI as a hierarchical tree where complex components are composed from simpler visual elements.

## Event System

Prism includes a flexible event system responsible for delivering input and lifecycle events throughout the UI tree.

Events are emitted continuously as the application runs. These can include events such as:

* mouse clicks
* pointer movement
* keyboard input
* frame updates (ticks)

Objects within the UI tree can listen for these events and react accordingly. Components can implement event handlers to process events and propagate them to their children when appropriate.

This event system forms the basis for all user interaction within frameworks built on Prism.

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

Prism uses **wgpu** through `wgpu_canvas` for rendering. As a result, Prism-based UI frameworks can target:

* Linux
* Windows
* macOS
* Android
* iOS
* Web

## Examples

Example applications demonstrating Prism are included in this repository. 

Frameworks built on Prism demonstrate how the core systems can be used to create structured UI environments. For example, **Pelican UI** builds on Prism to provide a full design system with predefined components and layouts.

Developers exploring Prism are encouraged to review these frameworks to understand how Prism's layout, rendering, and event systems can be composed into complete user interface frameworks.

## Discord

https://discord.gg/53ERRpS4S4

Join the Discord server to ask questions, discuss development, or share projects.