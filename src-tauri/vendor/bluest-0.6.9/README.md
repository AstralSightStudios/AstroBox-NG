This is the `bluest` crate (a cross-platform Bluetooth Low Energy library), version 0.6.9.
The upstream README is kept as `README.upstream.md`.

AstroBox carries one patch, in `src/corebluetooth/delegates.rs`: on macOS and iOS every event
of a peripheral (notifications, write-without-response readiness, discovery results, ...) goes
through a single `async_broadcast` channel shared by all receivers, with overflow enabled.
Upstream gives that channel a capacity of 16, so when the task draining a notification stream
falls behind for a moment (a busy runtime, the app being in the background), the oldest events
are dropped silently and the band's SAR frames are lost. The patch raises the capacity to 1024
for peripherals and 256 for the central. The backlog only fills while a receiver lags.

When upgrading bluest, check whether upstream has made the capacity configurable before
carrying this patch forward.
