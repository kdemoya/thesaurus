# Third-party components

The [MIT license](LICENSE) covers original Thesaurus code. Dependencies, bundled models, fonts, and tools retain their own licenses and copyright notices.

## Installer notice bundle

The desktop build runs `scripts/build-notices.sh [debug|release]` and includes the resulting `target/{profile}/notices/` directory in the app and alpha archive. The app's **Third-party licenses** menu item opens its readable index. The notice bundle includes:

- The original Thesaurus MIT license and full selected Rust license texts.
- Resolved package versions, source URLs, checksums, original license/NOTICE files and upstream authors metadata.
- Supplemental, hash-pinned upstream notices for crates that omit license text from their published packages.
- Exact unchanged source archives for MPL-2.0 dependencies, with source availability and extraction instructions. The generator verifies their source files against the archives and fails if local modifications need to be distributed.
- Font, model, native support code and Rust standard library notices.
- A machine-readable inventory and checksums for every notice, license and source archive in the bundle.

Generation uses the locked workspace graph for native macOS and browser WASM targets. It includes build dependencies and excludes development dependencies; some listed components are used during compilation and are not linked into the installed app. Tooling versions, supplemental notice provenance and exception handling are documented in [scripts/notices/README.md](scripts/notices/README.md). New licenses or missing original notices require review before an updated installer can be built.

## Assets and native components

| Component | Source and license | Notice handling |
| --- | --- | --- |
| JetBrains Mono font | [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), SIL Open Font License 1.1 | The font and its [OFL notice](apps/ui/fonts/OFL.txt) are in `apps/ui/fonts/`; the notice is also bundled. |
| whisper.cpp / ggml | [Pinned source](https://github.com/ggml-org/whisper.cpp/tree/d1be6fde11ac6e0407606b4e42fe72d34add8037), MIT, with bundled support-library notices | The verified source archive, original engine license and native support-library notices are bundled. |
| miniaudio, stb_vorbis, nlohmann/json and llamafile support code | [Pinned whisper.cpp source](https://github.com/ggml-org/whisper.cpp/tree/d1be6fde11ac6e0407606b4e42fe72d34add8037), package-specific MIT/MIT-0 or alternative licenses | Original source-header notices and the complete pinned source archive are bundled. |
| Bundled Whisper Base model | [Pinned converted model](https://huggingface.co/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1), MIT | The build verifies the model hash and bundles the original Whisper model license. |
| Rust standard library | [Rust](https://github.com/rust-lang/rust), MIT or Apache-2.0 with additional bundled support-code notices | The pinned toolchain's `COPYRIGHT-library.html` and accompanying license texts are bundled. |

Other models downloaded through Settings retain the licenses of their specific pinned sources. Downloaded build tools under `.tools/` are not installed with the app. Apple system frameworks are supplied by macOS.

The inventory preserves upstream notices and license declarations; it does not certify upstream licensing. The supplementary inventory explicitly documents packages whose published archives omit standalone license text and unavailable source-repository revisions. Review changes in those exceptions and in upstream notices, including the MCP SDK's license transition and objc2's Apple SDK explanation, before distributing updated builds.
