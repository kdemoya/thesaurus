// Platform boundary only. Rust owns sessions, consent, queueing and transcription.
import Foundation
import AVFoundation
import ScreenCaptureKit
import CoreMedia
import Darwin

let emitLock = NSLock()
func emit(_ value: [String: Any]) {
    guard let data = try? JSONSerialization.data(withJSONObject: value) else { return }
    emitLock.lock(); defer { emitLock.unlock() }
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([10]))
}
func fail(_ message: String) -> Never {
    emit(["event": "error", "message": message]); exit(1)
}
final class StopState: @unchecked Sendable {
    private let lock = NSLock()
    private var value = false
    func request() { lock.lock(); value = true; lock.unlock() }
    func requested() -> Bool { lock.lock(); defer { lock.unlock() }; return value }
}
final class RecordingClock: @unchecked Sendable {
    private let lock = NSLock()
    private var origin = ProcessInfo.processInfo.systemUptime
    func reset() { lock.lock(); origin = ProcessInfo.processInfo.systemUptime; lock.unlock() }
    func milliseconds() -> UInt64 { lock.lock(); defer { lock.unlock() }; return UInt64(max(0, ProcessInfo.processInfo.systemUptime - origin) * 1000) }
}
let pcmFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: 16000, channels: 1, interleaved: false)!
final class ChunkWriter {
    let folder: URL
    let track: String
    let limit: Int
    let started: RecordingClock
    var samples = [Int16]()
    var offset: UInt64 = 0
    var sequence = 0
    var converter: AVAudioConverter?
    init(folder: URL, track: String, seconds: Int, started: RecordingClock) {
        self.folder = folder; self.track = track; self.limit = seconds * 16000; self.started = started
    }
    func append(_ sample: CMSampleBuffer) throws {
        guard sample.isValid, let description = sample.formatDescription else { return }
        let format = AVAudioFormat(cmAudioFormatDescription: description)
        let count = CMSampleBufferGetNumSamples(sample)
        guard count > 0, count < 1_000_000,
              let input = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(count)) else { return }
        input.frameLength = AVAudioFrameCount(count)
        let status = CMSampleBufferCopyPCMDataIntoAudioBufferList(sample, at: 0, frameCount: Int32(count), into: input.mutableAudioBufferList)
        guard status == noErr else { throw NSError(domain: "audio sample conversion", code: Int(status)) }
        if converter?.inputFormat != format { converter = AVAudioConverter(from: format, to: pcmFormat) }
        guard let converter,
              let output = AVAudioPCMBuffer(pcmFormat: pcmFormat, frameCapacity: AVAudioFrameCount(Double(count) * 16000 / format.sampleRate) + 128) else {
            throw NSError(domain: "unsupported audio format", code: 1)
        }
        var supplied = false
        var error: NSError?
        let result = converter.convert(to: output, error: &error) { _, state in
            if supplied { state.pointee = .noDataNow; return nil }
            supplied = true; state.pointee = .haveData; return input
        }
        if result == .error { throw error ?? NSError(domain: "audio conversion", code: 2) }
        guard let floats = output.floatChannelData?[0] else { return }
        for i in 0..<Int(output.frameLength) {
            if samples.isEmpty { offset = started.milliseconds() }
            samples.append(floats[i].isFinite ? Int16(max(-1, min(1, floats[i])) * 32767) : 0)
            if samples.count == limit { try flush() }
        }
    }
    func flush() throws {
        guard !samples.isEmpty else { return }
        sequence += 1
        let filename = "\(track)-\(sequence).wav"
        let url = folder.appendingPathComponent(filename)
        let settings: [String: Any] = [AVFormatIDKey: kAudioFormatLinearPCM, AVSampleRateKey: 16000,
            AVNumberOfChannelsKey: 1, AVLinearPCMBitDepthKey: 16, AVLinearPCMIsFloatKey: false,
            AVLinearPCMIsBigEndianKey: false, AVLinearPCMIsNonInterleaved: false]
        guard let format = AVAudioFormat(settings: settings),
              let buffer = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: AVAudioFrameCount(samples.count)),
              let destination = buffer.int16ChannelData?[0] else { throw NSError(domain: "WAV format", code: 3) }
        buffer.frameLength = AVAudioFrameCount(samples.count)
        samples.withUnsafeBufferPointer { destination.update(from: $0.baseAddress!, count: $0.count) }
        // Close the file before announcing it so the WAV header is finalized.
        do { let file = try AVAudioFile(forWriting: url, settings: settings, commonFormat: .pcmFormatInt16, interleaved: true); try file.write(from: buffer) }
        emit(["event": "chunk", "file": filename, "track": track, "offset_ms": offset])
        samples.removeAll(keepingCapacity: true)
    }
}
final class MicrophoneOutput: NSObject, AVCaptureAudioDataOutputSampleBufferDelegate {
    let writer: ChunkWriter
    init(_ writer: ChunkWriter) { self.writer = writer }
    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        do { try writer.append(sampleBuffer) } catch { fail("Microphone audio conversion failed; recording paused") }
    }
}
@available(macOS 13.0, *)
final class SystemOutput: NSObject, SCStreamOutput, SCStreamDelegate {
    let writer: ChunkWriter
    init(_ writer: ChunkWriter) { self.writer = writer }
    func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .audio else { return }
        do { try writer.append(sampleBuffer) } catch { fail("System audio conversion failed; recording paused") }
    }
    func stream(_ stream: SCStream, didStopWithError error: Error) { fail("System audio became unavailable; recording paused") }
}
@main struct AudioBridge {
    static func main() async {
        umask(0o077)
        let args = CommandLine.arguments
        if args.count == 2 && args[1] == "inputs" {
            let devices = AVCaptureDevice.devices(for: .audio).map { ["id": $0.uniqueID, "name": $0.localizedName] }
            let permission: String
            switch AVCaptureDevice.authorizationStatus(for: .audio) {
            case .authorized: permission = "granted"
            case .notDetermined: permission = "not_requested"
            case .denied: permission = "denied"
            default: permission = "restricted"
            }
            var system = false
            if #available(macOS 13.0, *) { system = true }
            emit(["available": true, "system_audio_available": system, "microphone_permission": permission,
                  "microphones": devices, "detail": "Local recording; system audio includes all apps except Thesaurus. Permission is requested only on Start."])
            return
        }
        guard args.count == 6, args[1] == "record", let seconds = Int(args[3]), (10...60).contains(seconds) else {
            fail("Invalid recorder arguments")
        }
        let folder = URL(fileURLWithPath: args[2], isDirectory: true)
        let microphone = args[4] == "-" ? nil : args[4]
        let system = args[5] == "system"
        guard microphone != nil || system else { fail("No audio inputs selected") }
        if system {
            guard #available(macOS 13.0, *) else { fail("System audio requires macOS 13 or newer") }
        }
        // Parent death closes stdin. Never leave an orphaned recorder running.
        let stop = StopState()
        DispatchQueue.global().async {
            while let line = readLine() { if line == "stop" { stop.request(); return } }
            exit(0)
        }
        let started = RecordingClock()
        let micQueue = DispatchQueue(label: "thesaurus.microphone")
        let systemQueue = DispatchQueue(label: "thesaurus.system-audio")
        var microphoneSession: AVCaptureSession?
        var micOutput: MicrophoneOutput?
        var runtimeObserver: NSObjectProtocol?
        var disconnectObserver: NSObjectProtocol?
        if let microphone {
            switch AVCaptureDevice.authorizationStatus(for: .audio) {
            case .authorized:
                break
            case .notDetermined:
                guard await AVCaptureDevice.requestAccess(for: .audio) else {
                    fail("Microphone permission denied; enable it in macOS Privacy & Security and explicitly resume")
                }
            case .denied:
                fail("Microphone permission denied; enable it in macOS Privacy & Security and explicitly resume")
            default:
                fail("Microphone access is restricted by macOS; check Privacy & Security before resuming")
            }
            let selectedDevice = microphone == "system-default"
                ? AVCaptureDevice.default(for: .audio)
                : AVCaptureDevice.devices(for: .audio).first(where: { $0.uniqueID == microphone })
            guard let device = selectedDevice else { fail("Selected microphone is unavailable; choose an input again") }
            do {
                let session = AVCaptureSession()
                let input = try AVCaptureDeviceInput(device: device)
                let output = AVCaptureAudioDataOutput()
                let handler = MicrophoneOutput(ChunkWriter(folder: folder, track: "microphone", seconds: seconds, started: started))
                output.setSampleBufferDelegate(handler, queue: micQueue)
                guard session.canAddInput(input), session.canAddOutput(output) else { fail("Selected microphone cannot be opened") }
                session.addInput(input); session.addOutput(output)
                runtimeObserver = NotificationCenter.default.addObserver(forName: .AVCaptureSessionRuntimeError, object: session, queue: nil) { _ in fail("Microphone disconnected or capture failed; recording paused") }
                disconnectObserver = NotificationCenter.default.addObserver(forName: AVCaptureDevice.wasDisconnectedNotification, object: device, queue: nil) { _ in fail("Selected microphone disconnected; recording paused") }
                microphoneSession = session; micOutput = handler
            } catch { fail("Unable to configure selected microphone") }
        }
        // Configure every selected input before starting either one.
        var retainedStream: AnyObject?
        var retainedSystemOutput: AnyObject?
        if system {
            if #available(macOS 13.0, *) {
                do {
                    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
                    guard let display = content.displays.first else { fail("System audio requires an available display") }
                    let ownApps = content.applications.filter { $0.processID == getpid() || $0.bundleIdentifier == "dev.thesaurus.desktop" }
                    let filter = SCContentFilter(display: display, excludingApplications: ownApps, exceptingWindows: [])
                    let config = SCStreamConfiguration()
                    config.capturesAudio = true; config.excludesCurrentProcessAudio = true
                    config.sampleRate = 16000; config.channelCount = 1
                    config.width = 2; config.height = 2; config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
                    let handler = SystemOutput(ChunkWriter(folder: folder, track: "system", seconds: seconds, started: started))
                    let stream = SCStream(filter: filter, configuration: config, delegate: handler)
                    // No screen output is attached: no screen pixels are delivered or retained.
                    try stream.addStreamOutput(handler, type: .audio, sampleHandlerQueue: systemQueue)
                    if stop.requested() { return }
                    started.reset()
                    try await stream.startCapture()
                    retainedStream = stream; retainedSystemOutput = handler
                } catch { fail("System audio permission denied or input unavailable; check macOS Screen & System Audio Recording settings and explicitly resume") }
            }
        }
        if stop.requested() { return }
        if !system { started.reset() }
        microphoneSession?.startRunning()
        emit(["event": "ready"])
        // Inputs and delegates stay alive until Rust closes stdin or terminates us.
        while !stop.requested() {
            try? await Task.sleep(nanoseconds: 100_000_000)
            withExtendedLifetime((microphoneSession, micOutput, runtimeObserver, disconnectObserver, retainedStream, retainedSystemOutput)) {}
        }
        microphoneSession?.stopRunning()
        if #available(macOS 13.0, *), let stream = retainedStream as? SCStream { try? await stream.stopCapture() }
        do {
            try micQueue.sync { try micOutput?.writer.flush() }
            if #available(macOS 13.0, *), let output = retainedSystemOutput as? SystemOutput {
                try systemQueue.sync { try output.writer.flush() }
            }
        } catch { fail("Unable to finish final audio chunk") }
        emit(["event": "stopped"])

    }
}
