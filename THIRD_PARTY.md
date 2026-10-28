# Third-party components

The [MIT license](LICENSE) covers original Thesaurus code. Dependencies, bundled models, fonts, and tools retain their own licenses and copyright notices.

## Assets and bundled components

| Component | Source and license | Notice handling |
| --- | --- | --- |
| JetBrains Mono font | [JetBrains Mono](https://github.com/JetBrains/JetBrainsMono), SIL Open Font License 1.1 | The font and its [OFL notice](apps/ui/fonts/OFL.txt) are in `apps/ui/fonts/`. |
| whisper.cpp | [Pinned engine source](https://github.com/ggml-org/whisper.cpp/releases/tag/v1.9.5), MIT | `scripts/build-transcription.sh` verifies the pinned archive and copies its license to the generated transcription resources. |
| Bundled Whisper Base model | [Pinned converted model](https://huggingface.co/ggerganov/whisper.cpp/tree/5359861c739e955e79d9a303bcbc70fb988958b1), MIT | The build verifies the model hash and copies the Whisper model license to the generated transcription resources. |

Other models downloaded through Settings must be reviewed against their specific pinned sources before inclusion in a distributed product. Downloaded tools under `.tools/` and generated bundle resources are excluded from the source repository.

## Rust dependencies

`Cargo.lock` pins the Rust dependency graph. To inspect resolved package metadata:

```sh
mkdir -p .local/release-checks
./scripts/cargo.sh metadata --locked --format-version 1 \
  > .local/release-checks/cargo-metadata.json
```

Package license declarations are an inventory, not a complete distribution notice bundle. Before distributing an installer, generate and review notices and license texts for the dependencies actually shipped, including transitive native code, and include them in the app and release archive. Preserve any required source availability and modification notices for components using licenses such as MPL-2.0. Review dependency changes for new obligations.

The installer notice bundle and a user-accessible third-party notices view remain release requirements. This file does not certify that those distribution requirements have been completed.
