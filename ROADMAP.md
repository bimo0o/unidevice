# Lighting and device synchronization

Current: physically verified static active-stage RGB on Harpoon RGB PRO firmware
3.13, with state readback. Gradient, breathing and rainbow now implemented with
explicit start/stop, fresh DPI preservation, per-frame readback and fault stop.
See specs/rgb-effects.md. Shared multi-device synchronization remains future work.

Implemented first effect engine: static, breathing, smooth two-color cycle, rainbow.
Prefer onboard volatile effects if a verified protocol supports them. Software
animation needs a separate explicit enable/stop action, measured packet-rate limits,
serialized device traffic and a circuit breaker; no profile flash writes per frame.
The single-apply policy has an explicit RGB animation exception with tests. DPI
controls stop animation before applying; frames preserve freshly read DPI.

Later: one shared monotonic timeline/palette for multiple devices, independent
per-device capability checks. Devices with one RGB zone cannot render spatial waves.
Reconnection starts read-only; effects do not automatically resume on a replacement
device. Unknown models may join through verified protocol discovery; unknown
protocols stay read-only. No promise of universal writable RGB from a descriptor.
