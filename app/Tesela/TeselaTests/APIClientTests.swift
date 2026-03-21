import XCTest
@testable import Tesela

// MARK: - APIClientTests
// These tests are integration tests that require a running tesela-server.
// Mark them as requires-server so they can be skipped in CI.

final class APIClientTests: XCTestCase {
    // Integration tests — need server running at localhost:7474
    // Run manually: cargo run -p tesela-server

    func testHealthEndpoint() async throws {
        let client = APIClient()
        // Will fail (connection refused) if server is not running
        // XCTExpectFailure("Server may not be running in CI")
        let isHealthy = try await client.health()
        XCTAssertTrue(isHealthy)
    }

}
