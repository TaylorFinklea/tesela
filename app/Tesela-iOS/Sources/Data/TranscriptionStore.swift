import Foundation
import Combine
import SwiftUI

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
        states[model.id] = .downloading(progress: 0, bytesWritten: 0, totalBytes: model.sizeBytes)
        let task = session.downloadTask(with: model.downloadURL)
        task.taskDescription = model.id
        downloadTasks[model.id] = task
        taskToModel[task.taskIdentifier] = model.id
        task.resume()
    }

    func cancelDownload(_ modelId: String) {
        downloadTasks[modelId]?.cancel()
        downloadTasks.removeValue(forKey: modelId)
        states[modelId] = .available
        persistStateAsync()
    }

    func deleteModel(_ modelId: String) {
        let url = localURL(for: modelId)
        try? FileManager.default.removeItem(at: url)
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
        modelsDirectory.appendingPathComponent("\(modelId).bin")
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
            let url = localURL(for: model.id)
            if FileManager.default.fileExists(atPath: url.path) {
                let size = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? Int64) ?? 0
                rebuilt[model.id] = .downloaded(sizeOnDisk: size)
            } else {
                rebuilt[model.id] = .available
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
        // The temp file at `location` is only valid for the duration
        // of this delegate callback — move it synchronously.
        let dest = MainActor.assumeIsolated { self.localURL(for: modelId) }
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
