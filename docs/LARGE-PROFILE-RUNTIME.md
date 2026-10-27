# Large-profile native runtime checkpoint

Six ten-second release runs presented the native Export Profiles preview with 1,000 synthetic profiles. Three used an exactly 2,097,152-byte compact native document; three used a 174,857-byte document with exactly 32,768 structural tokens. The byte-heavy document has 6,011 tokens. Each retains opaque fields and uses a legacy model key, exercising the alias-aware preview without expanding the original backup.

The measured source is `ed25abdb75c6f8ec6fff85e765ccb5ce50f7b6f2`, built with locked offline dependencies. The exact release executable SHA256 is `9F10AF8B425CC384D521A02A51BA7BB857E119078AB33FB2E6A328342F93A06C`. This includes the source-derived assignment UI code, but the measured workload opens the profile export preview, not an assignment popup.

| Document | Run | Peak working set | Sampled peak private bytes | Peak paged memory |
| --- | ---: | ---: | ---: | ---: |
| Exact byte limit | 1 | 51,798,016 | 20,598,784 | 40,382,464 |
| Exact byte limit | 2 | 49,897,472 | 34,291,712 | 40,534,016 |
| Exact byte limit | 3 | 49,889,280 | 20,250,624 | 40,325,120 |
| Exact token limit | 1 | 38,416,384 | 17,526,784 | 25,935,872 |
| Exact token limit | 2 | 38,424,576 | 17,395,712 | 25,899,008 |
| Exact token limit | 3 | 38,490,112 | 18,006,016 | 25,931,776 |

All metrics are bytes. Every run remained below 100,000,000. Working set uses the observed process-tree high-water metric; private bytes are sampled and may miss brief peaks. Peak paged memory is retained as an additional conservative process metric. These runs observed one application process and no TCP connection. They do not prove that all possible network activity is absent.

Each native readiness report confirmed the requested view was matched, presented and visible, all three source fonts loaded, 1,000 profile rows available and the active row selected. The intrinsic query-only policy and query-only device handles were both reported active. The original profile files were byte-identical after each group of three runs. Synthetic names and opaque data contain no private user profile material.

Stock Razer user processes were temporarily isolated: 11 before, then two stable zero observations, and 15 after the configured hidden restart. Installed services and kernel drivers were retained and excluded from replacement-process totals. The uniform lighting worker, profile replay, mutation actions and policy writes were disabled by the intrinsic measurement policy.

Two earlier attempts against source `a0766e1`, executable `B4B716BDDCC8462A2953BCB6B23351DDFF37DC25F5E7DECBB7447C0D9B96E32B`, did not present the required view and are excluded. The first lacked a selected keyboard view with stock processes still running. The isolated retry exposed a genuine preview bug: resolving a legacy key enlarged an accepted boundary document. The fix previews the original backup through the existing alias-aware codec. The attempts also exposed reporting fields on Settings instead of the actual state snapshot; enforcement already existed, and the reporting location was corrected. No failed observation is relabeled as a passing measurement.

See [sanitized results and fixture hashes](validation/large-profile-native-memory.json), [bounded storage](NATIVE-STORAGE.md) and the separate [active-lighting checkpoint](ACTIVE-LIGHTING-RUNTIME.md). This checkpoint covers large-document query-only presentation. Assignment interaction, active effects with these documents, hotplug, sleep/wake, abrupt termination, replacement kernel components, full pixel parity and final-build/workflow memory acceptance remain separate requirements.
