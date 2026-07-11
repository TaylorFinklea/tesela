import Foundation
import Combine
import SwiftUI
import FluidAudio

/// Per-model state in the manage-models UI. Persisted under
/// `Application Support/TranscriptionModels/state.json`.
enum ModelState: Equatable, Codable {
    /// Not downloaded yet (only in the catalog).
    case available
    /// Download in flight. `progress` is 0.0–1.0; `bytesWritten` and
    /// `totalBytes` come from URLSession's progress callbacks.
    case downloading(progress: Double, bytesWritten: Int64, totalBytes: Int64)
    /// Fully downloaded and on disk.
    case downloaded(sizeOnDisk: Int64)
    /// Download was attempted and failed; carries a one-line message
    /// suitable for showing in the row.
    case failed(String)
}

/// Manages the catalog state on disk: which models are downloaded,
/// progress for in-flight downloads, which one is active. All file
/// I/O happens on the main actor for simplicity — model files are
/// large but writes are infrequent.
@MainActor
final class TranscriptionStore: NSObject, ObservableObject {
    @Published private(set) var states: [String: ModelState] = [:]
    @AppStorage("transcription.activeModelId") var activeModelId: String = ""

    private var downloadTasks: [String: URLSessionDownloadTask] = [:]
    private var taskToModel: [Int: String] = [:]
    /// In-flight FluidAudio (Parakeet) downloads, so they can be cancelled.
    private var parakeetTasks: [String: Task<Void, Never>] = [:]

    private lazy var modelsDirectory: URL = {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first!
        let dir = base.appendingPathComponent("TranscriptionModels", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }()

    private lazy var session: URLSession = {
        // Background-capable configuration so a download can continue
        // if the app gets suspended. iOS will resume the task on the
        // next app launch via the URLSession identifier.
        let config = URLSessionConfiguration.background(
            withIdentifier: "app.tesela.ios.transcription.downloads"
        )
        config.allowsCellularAccess = true
        config.sessionSendsLaunchEvents = true
        return URLSession(configuration: config, delegate: self, delegateQueue: nil)
    }()

    override init() {
        super.init()
        rehydrateFromDisk()
    }

    // MARK: - Public API

    func state(for modelId: String) -> ModelState {
        states[modelId] ?? .available
    }

    func startDownload(_ model: TranscriptionModel) {
        // Already downloading or downloaded — no-op.
        switch state(for: model.id) {
        case .downloading, .downloaded: return
        default: break
        }
        if model.family == .parakeet {
            startParakeetDownload(model)
            return
        }
        if model.family == .parakeetUnified {
            startParakeetUnifiedDownload(model)
            return
        }
        guard let url = model.downloadURL else {
            states[model.id] = .failed("No download URL for this model.")
            return
        }
        states[model.id] = .downloading(progress: 0, bytesWritten: 0, totalBytes: model.sizeBytes)
        let task = session.downloadTask(with: url)
        task.taskDescription = model.id
        downloadTasks[model.id] = task
        taskToModel[task.taskIdentifier] = model.id
        task.resume()
    }

    /// Parakeet downloads go through FluidAudio's `downloadAndLoad`,
    /// which fetches + caches the CoreML model set. It reports no
    /// progress, so the row shows an indeterminate state (totalBytes 0).
    private func startParakeetDownload(_ model: TranscriptionModel) {
        guard let version = fluidAudioVersion(model.parakeetVersion) else {
            states[model.id] = .failed("Unknown Parakeet version.")
            return
        }
        states[model.id] = .downloading(progress: 0, bytesWritten: 0, totalBytes: 0)
        let id = model.id
        let size = model.sizeBytes
        let cacheURL = Self.parakeetCacheURL(versionToken: model.parakeetVersion ?? "")
        parakeetTasks[id] = Task { [weak self] in
            do {
                _ = try await AsrModels.downloadAndLoad(to: cacheURL, version: version)
                guard let self, !Task.isCancelled else { return }
                self.states[id] = .downloaded(sizeOnDisk: size)
                if self.activeModelId.isEmpty { self.activeModelId = id }
                self.parakeetTasks.removeValue(forKey: id)
                self.persistStateAsync()
            } catch {
                guard let self, !Task.isCancelled else { return }
                self.states[id] = .failed(error.localizedDescription)
                self.parakeetTasks.removeValue(forKey: id)
                self.persistStateAsync()
            }
        }
    }

    /// Download the currently-configured Parakeet Unified tier without
    /// loading its ~600 MB CoreML encoder into memory from Settings.
    private func startParakeetUnifiedDownload(_ model: TranscriptionModel) {
        states[model.id] = .downloading(
            progress: 0,
            bytesWritten: 0,
            totalBytes: model.sizeBytes
        )
        let id = model.id
        let size = model.sizeBytes
        let tier = ParakeetUnifiedTier.active
        let cacheURL = Self.parakeetUnifiedCacheURL()
        let encoderFile = ModelNames.ParakeetUnified.streamingEncoderFile(
            precision: .int8,
            contextSuffix: tier.contextSuffix
        )
        parakeetTasks[id] = Task { [weak self] in
            guard let store = self else { return }
            do {
                try await ModelHub.download(
                    .parakeetUnified,
                    to: cacheURL,
                    additionalModelNames: [encoderFile]
                ) { progress in
                    let fraction = Self.parakeetUnifiedDownloadFraction(
                        progress.fractionCompleted
                    )
                    Task { @MainActor [weak store] in
                        guard let store,
                              case .downloading = store.state(for: id)
                        else { return }
                        store.states[id] = .downloading(
                            progress: fraction,
                            bytesWritten: Int64(Double(size) * fraction),
                            totalBytes: size
                        )
                    }
                }
                guard !Task.isCancelled else { return }
                store.states[id] = .downloaded(sizeOnDisk: size)
                if store.activeModelId.isEmpty { store.activeModelId = id }
                store.parakeetTasks.removeValue(forKey: id)
                store.persistStateAsync()
            } catch {
                guard !Task.isCancelled else { return }
                store.states[id] = .failed(error.localizedDescription)
                store.parakeetTasks.removeValue(forKey: id)
                store.persistStateAsync()
            }
        }
    }

    func cancelDownload(_ modelId: String) {
        downloadTasks[modelId]?.cancel()
        downloadTasks.removeValue(forKey: modelId)
        parakeetTasks[modelId]?.cancel()
        parakeetTasks.removeValue(forKey: modelId)
        states[modelId] = .available
        persistStateAsync()
    }

    func deleteModel(_ modelId: String) {
        if let model = TranscriptionCatalog.find(modelId), model.family == .parakeet {
            let dir = Self.parakeetCacheURL(versionToken: model.parakeetVersion ?? "")
            try? FileManager.default.removeItem(at: dir)
        } else if let model = TranscriptionCatalog.find(modelId), model.family == .parakeetUnified {
            // Removes every downloaded tier's encoder along with the
            // shared decoder/joint/vocab files — deleting the family
            // means deleting the whole shared cache dir.
            try? FileManager.default.removeItem(at: Self.parakeetUnifiedCacheURL())
        } else {
            try? FileManager.default.removeItem(at: localURL(for: modelId))
        }
        states[modelId] = .available
        if activeModelId == modelId {
            activeModelId = ""
        }
        persistStateAsync()
    }

    func activate(_ modelId: String) {
        guard case .downloaded = state(for: modelId) else { return }
        // Block activation for models we can't actually run yet.
        // The Manage Models UI hides the Set Active button for these,
        // but a programmatic call could still get here.
        if let model = TranscriptionCatalog.find(modelId), !model.inferenceSupported {
            return
        }
        activeModelId = modelId
    }

    func localURL(for modelId: String) -> URL {
        Self.modelFileURL(for: modelId)
    }

    /// Destination file for a model. `nonisolated` + `static` so the
    /// background URLSession delegate can compute it without hopping to
    /// the main actor — `MainActor.assumeIsolated` from the delegate
    /// queue is a hard crash.
    nonisolated static func modelFileURL(for modelId: String) -> URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first ?? FileManager.default.temporaryDirectory
        let dir = base.appendingPathComponent("TranscriptionModels", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir.appendingPathComponent("\(modelId).bin")
    }

    /// Cache directory for a Parakeet version's CoreML model set —
    /// passed to FluidAudio's `downloadAndLoad(to:)` so the app owns
    /// the files (and `deleteModel` can remove them).
    nonisolated static func parakeetCacheURL(versionToken: String) -> URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first ?? FileManager.default.temporaryDirectory
        let dir = base
            .appendingPathComponent("TranscriptionModels", isDirectory: true)
            .appendingPathComponent("parakeet-\(versionToken)", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    /// Base cache directory passed to FluidAudio. FluidAudio appends its
    /// `Repo.parakeetUnified.folderName` below this directory.
    nonisolated static func parakeetUnifiedCacheURL() -> URL {
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first ?? FileManager.default.temporaryDirectory
        let dir = base
            .appendingPathComponent("TranscriptionModels", isDirectory: true)
            .appendingPathComponent("parakeet-unified", isDirectory: true)
        try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        return dir
    }

    /// Location of a downloaded tier's encoder after FluidAudio appends its
    /// repository directory to the caller-supplied cache base.
    nonisolated static func parakeetUnifiedEncoderURL(
        baseDirectory: URL,
        tier: ParakeetUnifiedTier
    ) -> URL {
        let encoderFile = ModelNames.ParakeetUnified.streamingEncoderFile(
            precision: .int8,
            contextSuffix: tier.contextSuffix
        )
        return baseDirectory
            .appendingPathComponent(Repo.parakeetUnified.folderName, isDirectory: true)
            .appendingPathComponent(encoderFile, isDirectory: true)
    }

    /// `ModelHub.download` reports the download half of FluidAudio's combined
    /// download-and-compile progress range. This screen does not compile/load,
    /// so expand that 0...0.5 phase to the full 0...1 range.
    nonisolated static func parakeetUnifiedDownloadFraction(_ fraction: Double) -> Double {
        min(max(fraction * 2, 0), 1)
    }

    // MARK: - Persistence

    private struct DiskState: Codable {
        var states: [String: ModelState]
    }

    private var stateFileURL: URL {
        modelsDirectory.appendingPathComponent("state.json")
    }

    private func persistStateAsync() {
        let snapshot = DiskState(states: states)
        let url = stateFileURL
        Task.detached(priority: .utility) {
            if let data = try? JSONEncoder().encode(snapshot) {
                try? data.write(to: url, options: .atomic)
            }
        }
    }

    private func rehydrateFromDisk() {
        // Walk what's actually on disk to recover ground truth even
        // if the JSON file is stale.
        var rebuilt: [String: ModelState] = [:]
        for model in TranscriptionCatalog.all {
            if model.family == .parakeet {
                // Parakeet is "downloaded" when FluidAudio's cache dir
                // for that version has files in it.
                let dir = Self.parakeetCacheURL(versionToken: model.parakeetVersion ?? "")
                let contents = (try? FileManager.default.contentsOfDirectory(atPath: dir.path)) ?? []
                rebuilt[model.id] = contents.isEmpty
                    ? .available
                    : .downloaded(sizeOnDisk: model.sizeBytes)
            } else if model.family == .parakeetUnified {
                // "Downloaded" means the ACTIVE tier's encoder file exists
                // — switching tiers in Settings can flip this back to
                // `.available` even though a different tier is still
                // cached (matches ParakeetUnifiedTier's per-tier download
                // model). Uses FluidAudio's own filename convention
                // directly so this can't drift from what `loadModels`
                // actually writes.
                let tier = ParakeetUnifiedTier.active
                let encoderPath = Self.parakeetUnifiedEncoderURL(
                    baseDirectory: Self.parakeetUnifiedCacheURL(),
                    tier: tier
                )
                rebuilt[model.id] = FileManager.default.fileExists(atPath: encoderPath.path)
                    ? .downloaded(sizeOnDisk: model.sizeBytes)
                    : .available
            } else {
                let url = localURL(for: model.id)
                if FileManager.default.fileExists(atPath: url.path) {
                    let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int64) ?? 0
                    rebuilt[model.id] = .downloaded(sizeOnDisk: size)
                } else {
                    rebuilt[model.id] = .available
                }
            }
        }
        states = rebuilt
        // Pick an active model if one isn't set and we have a download
        if activeModelId.isEmpty,
           let firstReady = rebuilt.first(where: {
               if case .downloaded = $0.value { return true } else { return false }
           })?.key {
            activeModelId = firstReady
        }
        persistStateAsync()
    }
}

// MARK: - URLSession delegate (download progress + completion)

extension TranscriptionStore: URLSessionDownloadDelegate {
    nonisolated func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didWriteData bytesWritten: Int64,
        totalBytesWritten: Int64,
        totalBytesExpectedToWrite: Int64
    ) {
        let modelId = downloadTask.taskDescription ?? ""
        let total = totalBytesExpectedToWrite > 0 ? totalBytesExpectedToWrite : 1
        let progress = Double(totalBytesWritten) / Double(total)
        Task { @MainActor in
            self.states[modelId] = .downloading(
                progress: progress,
                bytesWritten: totalBytesWritten,
                totalBytes: totalBytesExpectedToWrite
            )
        }
    }

    nonisolated func urlSession(
        _ session: URLSession,
        downloadTask: URLSessionDownloadTask,
        didFinishDownloadingTo location: URL
    ) {
        let modelId = downloadTask.taskDescription ?? ""
        // URLSession reports an HTTP error response as a "finished"
        // download whose file is the error body — reject non-2xx so a
        // 404 page never gets saved as a model (and surfaces honestly).
        if let http = downloadTask.response as? HTTPURLResponse,
           !(200..<300).contains(http.statusCode) {
            Task { @MainActor in
                self.states[modelId] = .failed("Download failed (HTTP \(http.statusCode))")
                self.downloadTasks.removeValue(forKey: modelId)
                self.persistStateAsync()
            }
            return
        }
        // The temp file at `location` is only valid for the duration of
        // this callback — move it synchronously. The destination is
        // computed `nonisolated`; this runs on URLSession's background
        // delegate queue, not the main actor.
        let dest = Self.modelFileURL(for: modelId)
        try? FileManager.default.removeItem(at: dest)
        do {
            try FileManager.default.moveItem(at: location, to: dest)
            let size = (try? FileManager.default.attributesOfItem(atPath: dest.path)[.size] as? Int64) ?? 0
            Task { @MainActor in
                self.states[modelId] = .downloaded(sizeOnDisk: size)
                self.downloadTasks.removeValue(forKey: modelId)
                // Auto-activate the first downloaded model so the
                // user doesn't have to dig back into settings.
                if self.activeModelId.isEmpty {
                    self.activeModelId = modelId
                }
                self.persistStateAsync()
            }
        } catch {
            Task { @MainActor in
                self.states[modelId] = .failed("Couldn't save: \(error.localizedDescription)")
                self.downloadTasks.removeValue(forKey: modelId)
                self.persistStateAsync()
            }
        }
    }

    nonisolated func urlSession(
        _ session: URLSession,
        task: URLSessionTask,
        didCompleteWithError error: Error?
    ) {
        guard let error else { return }
        // URLSession cancellation surfaces here as well; ignore those.
        if (error as NSError).code == NSURLErrorCancelled { return }
        let modelId = task.taskDescription ?? ""
        Task { @MainActor in
            self.states[modelId] = .failed(error.localizedDescription)
            self.downloadTasks.removeValue(forKey: modelId)
            self.persistStateAsync()
        }
    }
}
