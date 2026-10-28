import Foundation
import Vision
import CoreML
// This native bridge only recognizes one explicitly selected local PNG.
func recognize(_ file: URL, cpuOnly: Bool, compatibilityRevision: Bool = false) throws -> String {
    let request = VNRecognizeTextRequest()
    if compatibilityRevision {
        // Some OS/device combinations advertise current-revision CPU support
        // but fail to create its precompiled compute operation. Revision 2
        // remains a runtime-discoverable local compatibility option.
        request.revision = 2
    }
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false
    if cpuOnly {
        #if compiler(>=5.9)
        if #available(macOS 14.0, *) {
            let stages = try request.supportedComputeStageDevices
            guard !stages.isEmpty else {
                throw NSError(domain: "ThesaurusOCR", code: 1,
                    userInfo: [NSLocalizedDescriptionKey: "No CPU stages available for local OCR"])
            }
            for (stage, devices) in stages {
                guard let cpu = devices.first(where: {
                    if case .cpu = $0 { return true }
                    return false
                }) else {
                    throw NSError(domain: "ThesaurusOCR", code: 1,
                        userInfo: [NSLocalizedDescriptionKey: "No CPU device available for local OCR"])
                }
                request.setComputeDevice(cpu, for: stage)
            }
        } else {
            request.usesCPUOnly = true
        }
        #else
        // Older Apple toolchains do not know the newer compute-device APIs,
        // even inside a runtime availability check.
        request.usesCPUOnly = true
        #endif
    }
    let handler = VNImageRequestHandler(url: file, options: [:])
    try handler.perform([request])
    return (request.results ?? []).compactMap { $0.topCandidates(1).first?.string }
        .joined(separator: "\n")
}

do {
    let compatibility = CommandLine.arguments.count == 3
        && CommandLine.arguments[2] == "--cpu-compatibility"
    guard CommandLine.arguments.count == 2 || compatibility else {
        throw NSError(domain: "ThesaurusOCR", code: 2,
            userInfo: [NSLocalizedDescriptionKey: "Expected a local PNG and optional CPU compatibility mode"])
    }
    let file = URL(fileURLWithPath: CommandLine.arguments[1])
    let text: String
    if compatibility {
        guard VNRecognizeTextRequest.supportedRevisions.contains(2) else {
            throw NSError(domain: "ThesaurusOCR", code: 4,
                userInfo: [NSLocalizedDescriptionKey: "Local OCR compatibility revision unavailable"])
        }
        text = try recognize(file, cpuOnly: true, compatibilityRevision: true)
    } else {
        do {
            text = try recognize(file, cpuOnly: false)
        } catch {
            // Retry the current revision on CPU. The Rust provider owns a
            // separate-process compatibility retry: a failed precompiled
            // operation can leave this process unable to use another revision.
            text = try recognize(file, cpuOnly: true)
        }
    }
    FileHandle.standardOutput.write(Data(text.utf8))
} catch {
    // A failed request must exit normally rather than trapping with a crash dump.
    FileHandle.standardError.write(Data("Local Vision OCR failed: \(error.localizedDescription)\n".utf8))
    exit(1)
}
