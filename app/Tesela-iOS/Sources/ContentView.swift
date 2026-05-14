import SwiftUI

struct ContentView: View {
    private let coreVersion = teselaSyncVersion()
    private let schemaVersion = syncSchemaVersion()
    private let sampleDeviceId = generateDeviceIdHex()

    var body: some View {
        VStack(spacing: 16) {
            Text("Tesela iOS")
                .font(.largeTitle)
                .bold()

            VStack(spacing: 4) {
                Text("Rust core version")
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Text(coreVersion)
                    .font(.system(.title2, design: .monospaced))
            }

            VStack(spacing: 4) {
                Text("Sync schema")
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Text("v\(schemaVersion)")
                    .font(.system(.title2, design: .monospaced))
            }

            Divider().padding(.vertical, 8)

            VStack(spacing: 4) {
                Text("Sample device id")
                    .font(.headline)
                    .foregroundStyle(.secondary)
                Text(sampleDeviceId)
                    .font(.system(.caption, design: .monospaced))
                    .multilineTextAlignment(.center)
                    .padding(.horizontal)
            }
        }
        .padding()
    }
}

#Preview {
    ContentView()
}
