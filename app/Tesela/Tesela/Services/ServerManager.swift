import Foundation

/// Manages the lifecycle of the tesela-server child process.
/// Launches on app start, kills on app quit, monitors health.
@MainActor
final class ServerManager {
    static let shared = ServerManager()

    private var process: Process?
    private let serverPort = 7474
    private let healthURL = URL(string: "http://127.0.0.1:7474/health")!

    private init() {}

    /// Start the server if not already running. Returns true if server is healthy.
    func ensureRunning() async -> Bool {
        // Check if server is already running (e.g., from terminal or LaunchAgent)
        if await isHealthy() { return true }

        // Find the binary
        guard let binary = findServerBinary() else {
            print("[ServerManager] tesela-server binary not found")
            return false
        }

        // Find the mosaic directory
        guard let mosaic = findMosaicDirectory() else {
            print("[ServerManager] No mosaic directory found")
            return false
        }

        print("[ServerManager] Starting \(binary) in \(mosaic)")

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binary)
        proc.currentDirectoryURL = URL(fileURLWithPath: mosaic)
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice

        // Kill server when app terminates
        proc.terminationHandler = { p in
            print("[ServerManager] Server exited with code \(p.terminationStatus)")
        }

        do {
            try proc.run()
            process = proc
        } catch {
            print("[ServerManager] Failed to start server: \(error)")
            return false
        }

        // Wait for server to become healthy (up to 5 seconds)
        for _ in 0..<50 {
            try? await Task.sleep(for: .milliseconds(100))
            if await isHealthy() {
                print("[ServerManager] Server is healthy")
                return true
            }
        }

        print("[ServerManager] Server did not become healthy in time")
        return false
    }

    /// Stop the server process
    func stop() {
        guard let proc = process, proc.isRunning else { return }
        proc.terminate()
        process = nil
        print("[ServerManager] Server stopped")
    }

    /// Check if the server responds to /health
    private func isHealthy() async -> Bool {
        do {
            let (data, response) = try await URLSession.shared.data(from: healthURL)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else { return false }
            let json = try? JSONSerialization.jsonObject(with: data) as? [String: String]
            return json?["status"] == "ok"
        } catch {
            return false
        }
    }

    /// Search common locations for the tesela-server binary
    private func findServerBinary() -> String? {
        let candidates = [
            // Cargo install location
            NSHomeDirectory() + "/.cargo/bin/tesela-server",
            // Next to the app binary
            Bundle.main.bundleURL
                .deletingLastPathComponent()
                .appendingPathComponent("tesela-server").path,
            // Homebrew
            "/opt/homebrew/bin/tesela-server",
            "/usr/local/bin/tesela-server",
        ]

        for path in candidates {
            if FileManager.default.isExecutableFile(atPath: path) {
                return path
            }
        }

        // Try $PATH via `which`
        let which = Process()
        which.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        which.arguments = ["tesela-server"]
        let pipe = Pipe()
        which.standardOutput = pipe
        try? which.run()
        which.waitUntilExit()
        let output = String(data: pipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)?
            .trimmingCharacters(in: .whitespacesAndNewlines)
        if let path = output, !path.isEmpty, FileManager.default.isExecutableFile(atPath: path) {
            return path
        }

        return nil
    }

    /// Find the mosaic directory (look for .tesela/ folder)
    private func findMosaicDirectory() -> String? {
        let candidates = [
            NSHomeDirectory() + "/z-temp/tesela-test",
            NSHomeDirectory() + "/tesela",
            NSHomeDirectory() + "/Documents/tesela",
        ]

        for path in candidates {
            let teselaDir = (path as NSString).appendingPathComponent(".tesela")
            if FileManager.default.fileExists(atPath: teselaDir) {
                return path
            }
        }

        return nil
    }
}
