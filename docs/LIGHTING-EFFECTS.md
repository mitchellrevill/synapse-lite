# Firmware effect boundary

The Lighting endpoint supports a source-derived firmware subset: Static, Spectrum, fixed-color Breathing, Reactive and fixed-color Starlight. Reactive/Starlight retain exact duration selectors1..3. Newly authored colors require #RRGGBB; random settings, Wave, a black second color and unrepresented source choices fail explicitly before hardware access. The twelve UI choices do not imply a complete host animation engine.

The handler checks exact device/profile identity, lighting ownership, aggregate store budget and a native snapshot of known competing Synapse processes. Enumeration failure refuses the command. Firmware effects require existing Normal Mode0; no mode setter or acquisition is implemented. The installed keyboard reports Driver Mode3. Source Static now has a separate [host-frame bridge](HOST-STATIC.md) in that mode; other listed firmware effects still refuse Driver3 application.

Writes use the existing native LED5 effect/color writer, also used when reapplying the saved profile. A fresh source profile1/region0 effect GET must match selectors, effect, flags, rate, color count and RGB data within the declared reply length before saving. Undeclared padding cannot verify a color. A matching final state is a per-action check, not proof of universal selector aliasing or visible animation parity. SET acknowledgement validation remains strict, and a failed SET/readback/save is not automatically retried. Such failures report that hardware may have changed while retaining the prior profile.

The staged profile retains brightness/policy intent and unknown Zone/source fields. Quick effects replace native per-key colors as in the upstream controls. Unsupported source settings remain available in imported data without silently acquiring an approximate projection.

Simulator tests cover exact packets, durable native projection, mode/owner/budget/profile refusals, failed readback, absent selector aliasing and declared-short replies containing apparently matching bytes only in padding. No physical effect write has been performed for this endpoint. Firmware output is distinct from Synapse's host animation implementation; successful responses explicitly report host_animation_parity=false.

Protocol facts come from the recovered lighting-engine artifact main.a383ea9e.js, SHA256 c82c7074e5da8abcffbff95f23a6b962c1e14b04586715b9d4b7a6f11fe9e6d2: conversion module6394 and the common effect packet helper. Raw vendor source remains private research material.
