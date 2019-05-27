My goal is to create a simple EBML and Matroska (MKV) file parser.

### What I Learned

- More detailed knowledge about the structure of EBML and Matroska file format.
- How to model the structure with Rust types (`enum` and `struct`).
- How to organize the parser implementation in a way that is easy to comprehend
- How to use Rust macros to reduce code repetition and improve readability.

### Afterthoughts

- nom is not flexible enough to handle the structure of EBML. I ended up with using only some basic functionalities instead of relying on various parse combinators. For this project, hand-crafted and custom macros lead to easier-to-understand code.

### TODO

- Loading MKV file content incrementally instead loading it entirely in memory for parsing.
- Implement all parser for all level-1 elements.