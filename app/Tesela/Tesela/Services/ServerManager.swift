import Foundation
import os.log

private let logger = Logger(subsystem: "com.tesela.ServerManager", category: "ServerManager")

/// Manages the lifecycle of the tesela-server child process.
/// Launches on app start, kills on app quit, monitors health.
@MainActor
final class ServerManager {
    static let shared = ServerManager()

    private var process: Process?
    private let serverPort = 7474
    private let healthURL = URL(string: "http://127.0.0.1:7474/health")!
    private let healthCheckIntervalMs: UInt64 = 100
    private let healthCheckMaxAttempts = 50

    private init() {}

    /// Start the server if not already running. Returns true if server is healthy.
    func ensureRunning() async -> Bool {
        // Check if server is already running (e.g., from terminal or LaunchAgent)
        if await isHealthy() { return true }

        // Find the binary
        guard let binary = findServerBinary() else {
            logger.error("tesela-server binary not found")
            return false
        }

        // Find the mosaic directory
        guard let mosaic = findMosaicDirectory() else {
            logger.error("No mosaic directory found")
            return false
        }

        logger.info("Starting \(binary) in \(mosaic)")

        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: binary)
        proc.currentDirectoryURL = URL(fileURLWithPath: mosaic)
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice

        // Kill server when app terminates
        proc.terminationHandler = { p in
            logger.info("Server exited with code \(p.terminationStatus)")
        }

        do {
            try proc.run()
            process = proc
        } catch {
            logger.error("Failed to start server: \(error.localizedDescription)")
            return false
        }

        // Wait for server to become healthy (up to 5 seconds)
        for _ in 0..<healthCheckMaxAttempts {
            do {
                try await Task.sleep(for: .milliseconds(healthCheckIntervalMs))
            } catch {
                logger.debug("Health check sleep failed: \(error.localizedDescription)")
            }
            if await isHealthy() {
                logger.info("Server is healthy")
                return true
            }
        }

        logger.warning("Server did not become healthy in time")
        return false
    }

    /// Stop the server process
    func stop() {
        guard let proc = process, proc.isRunning else { return }
        proc.terminate()
        process = nil
        logger.info("Server stopped")
    }

    /// Check if the server responds to /health
    private func isHealthy() async -> Bool {
        do {
            let (data, response) = try await URLSession.shared.data(from: healthURL)
            guard let http = response as? HTTPURLResponse, http.statusCode == 200 else { return false }
            let json: [String: String]?
            do {
                json = try JSONSerialization.jsonObject(with: data) as? [String: String]
            } catch {
                logger.debug("Health check JSON parsing failed: \(error.localizedDescription)")
                json = nil
            }
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
        do {
            try which.run()
        } catch {
            logger.debug("Failed to run which command: \(error.localizedDescription)")
        }
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
