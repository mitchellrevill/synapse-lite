# Installed input-mapping readback

The bounded `input_readback` example sends only discovery and input GET requests. It queries one input on each inspected model, slots0/1 and numerical layers0/1. It never sets mappings, sends input events, changes modes or edits profiles/drivers. The [redacted observations](validation/input-readback.json) retain exact replies and executable identities.

Ornata025D/interface2 returned `NotSupported` for all four tested key31 GET02:8D requests. This is evidence for those requests, not a proof that every possible keyboard mapping mechanism is unavailable.

The first enumeration found three wired00C4/interface0 collections and correctly skipped the ambiguous mouse. A second mouse-only probe selected the documented usage-page1/usage2 collection. Input4 GET02:8C returned layer1 for requests naming either layer0 or layer1 in both slots. Numerical selectors therefore mismatched for layer0. A matching numerical layer1 reply does not establish normal/Hypershift semantics or safe writes.

Stock user processes were temporarily isolated for the queries and restarted afterward:11 before,0 isolated,15 after each run. Services and drivers stayed running. No remapping or input recovery was exercised.

Core input getters now validate returned slot/input/layer before decoding an action. A live mapping setter requires a valid baseline read; failed or mismatched GETs no longer fall through to a SET. Simulator regressions alter otherwise-valid reply selectors and verify that no write follows, while exact unchanged reads remain no-ops and valid changed actions retain the existing path. These checks do not establish complete mapping compatibility or recovery.

Build with `cargo build -p secbird-core --example input_readback`, then supply a fresh report path and `--read-only`. `--mouse-only` narrows the queries. Hardware probes are explicit tools and do not run as ordinary tests. Preserve competing-owner coordination and a restart strategy when using them; there is no automatic driver/client ownership lease.
