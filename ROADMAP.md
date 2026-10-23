# Lighting and device synchronization

Current: physically verified static active-stage RGB on Harpoon RGB PRO firmware
3.13, with state readback. Effects are not implemented in this build.

Next: a capability-driven effect engine: static, breathing, smooth color cycle.
Prefer onboard volatile effects if a verified protocol supports them. Software
animation needs a separate explicit enable/stop action, measured packet-rate limits,
serialized device traffic and a circuit breaker; no profile flash writes per frame.
The current single-apply safety policy must be extended and tested before enabling
periodic RGB changes. DPI controls must not race animation writes or restore stale DPI.

Later: one shared monotonic timeline/palette for multiple devices, independent
per-device capability checks. Devices with one RGB zone cannot render spatial waves.
Reconnection starts read-only; effects do not automatically resume on a replacement
device. Unknown models may join through verified protocol discovery; unknown
protocols stay read-only. No promise of universal writable RGB from a descriptor.
