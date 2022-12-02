# NEAR chunked collections

[<img alt="github" src="https://img.shields.io/badge/github-austinabell/near-chunked-collections-8da0cb?style=for-the-badge&labelColor=555555&logo=github" height="20">](https://github.com/austinabell/near-chunked-collections)
<!-- [<img alt="crates.io" src="https://img.shields.io/crates/v/near-chunked-collections.svg?style=for-the-badge&color=fc8d62&logo=rust" height="20">](https://crates.io/crates/near-chunked-collections)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-near-chunked-collections-66c2a5?style=for-the-badge&labelColor=555555&logo=docs.rs" height="20">](https://docs.rs/near-chunked-collections) -->
[<img alt="build status" src="https://img.shields.io/github/workflow/status/austinabell/near-chunked-collections/CI/main?style=for-the-badge" height="20">](https://github.com/austinabell/near-chunked-collections/actions?query=branch%3Amain)

This crate aims to provide chunked storage collections for NEAR smart contracts. The benefit of this is more efficient gas usage in cases where the cost savings from performing fewer overall reads is greater than the potentially increased number of bytes written per element.

Visually and over-simplified, this will look something like this:

old
```
    root
 / / | \ \
a  b c  d e
```

new
```
    root
  /  |   \
[ab] [cd] [e]
```

Potential Data structures:
- [ ] Chunked Vector
- [ ] Chunked VecDeque
- [ ] Chunked FreeList
  - This is useful to have some stable indexing with the ability to remove elements without requiring a swap remove or shift
- [ ] Chunked Map
  - This is more complex because it's not clear how keys are grouped. Potentially similar to [IPLD HAMT](https://ipld.io/specs/advanced-data-layouts/hamt/spec/) where it's based on the key's hash prefix but strips away the hash linking bloat and makes storage accesses stable to avoid recursively updating parent nodes

Goals:
- More optimized for large-scale operations
  - Benchmarks for different sizes of keys/values, operations, and distribution
- APIs matching the `std` Rust counterparts
  - Exception could be made if we want to wrap the accesses in a guard pattern to be able to drop values in memory when not used anymore
  - This could come at the cost of performance; maybe the intention for this is to be more low-level and have an API that is more error-prone
- Generic over size of chunks for every collection

Nice-to-have:
- Minimal dependencies, would like to eventually avoid using a high-level lib like the [NEAR SDK](https://github.com/near/near-sdk-rs) to make this more usable in low-level applications
  - Ideal if `no_std` compat
- Blockchain/storage agnostic
  - If the storage access is abstracted to a bytes key-value storage access, it can be used in more contexts
