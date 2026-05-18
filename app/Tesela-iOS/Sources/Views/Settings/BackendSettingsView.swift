import SwiftUI

/// Settings → Backend. Lets the user pick mock vs HTTP and edit the
/// `tesela-server` URL. Saving triggers a `mosaic.refresh(...)` so the
/// UI immediately reflects the new source.
struct BackendSettingsView: View {
    @ObservedObject var backend: BackendSettings
    @ObservedObject var mosaic: MockMosaicService

    @Environment(\.theme) private var theme
    @State private var pickerMode: BackendSettings.Mode
    @State private var urlField: String
    @State private var isReloading: Bool = false

    init(backend: BackendSettings, mosaic: MockMosaicService) {
        self.backend = backend
        self.mosaic = mosaic
        self._pickerMode = State(initialValue: backend.mode)
        self._urlField = State(initialValue: backend.serverURL)
    }

    var body: some View {
        Form {
            Section {
                Picker("Source", selection: $pickerMode) {
                    Text("Mock data").tag(BackendSettings.Mode.mock)
                    Text("Local server (HTTP)").tag(BackendSettings.Mode.http)
                }
                .pickerStyle(.segmented)
            } header: {
                Text("Mosaic source")
            } footer: {
                Text("Mock is a built-in snapshot for design previews. HTTP hits a tesela-server you're running on the same machine or LAN.")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }

            if pickerMode == .http {
                Section {
                    TextField("Server URL", text: $urlField)
                        .font(.system(.body, design: .monospaced))
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .keyboardType(.URL)
                } header: {
                    Text("URL")
                } footer: {
                    Text("Simulator: use 127.0.0.1. Real device: use the host's LAN address (e.g. 192.168.1.42).")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
            }

            Section {
                connectionRow
            }

            Section {
                Button {
                    Task { await save() }
                } label: {
                    HStack {
                        if isReloading {
                            ProgressView()
                                .tint(theme.bg)
                        }
                        Text(isReloading ? "Refreshing…" : "Save & refresh")
                            .font(.system(size: 14, weight: .semibold))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                    .foregroundStyle(theme.bg)
                    .background(theme.accentPrimary)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
                .listRowInsets(EdgeInsets(top: 8, leading: 12, bottom: 8, trailing: 12))
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Backend")
        .navigationBarTitleDisplayMode(.inline)
    }

    private var connectionRow: some View {
        HStack(spacing: 10) {
            statusDot
            VStack(alignment: .leading, spacing: 2) {
                Text(statusLabel)
                    .font(.system(size: 13, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                if let detail = statusDetail {
                    Text(detail)
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .lineLimit(2)
                }
            }
            Spacer()
        }
        .padding(.vertical, 4)
    }

    private var statusDot: some View {
        let color: Color = {
            switch mosaic.connection {
            case .ready:      return theme.typeQuery
            case .connecting: return theme.typeNote
            case .failed:     return theme.typeTask
            case .idle:       return theme.fgFaint
            }
        }()
        return Circle().fill(color).frame(width: 10, height: 10)
    }

    private var statusLabel: String {
        switch mosaic.connection {
        case .idle:       return backend.mode == .mock ? "Mock data" : "Not yet connected"
        case .connecting: return "Connecting…"
        case .ready:      return "Connected"
        case .failed:     return "Connection failed"
        }
    }

    private var statusDetail: String? {
        switch mosaic.connection {
        case .failed(let msg): return msg
        case .ready:           return backend.serverURL
        case .connecting:      return backend.serverURL
        default:               return nil
        }
    }

    @MainActor
    private func save() async {
        backend.mode = pickerMode
        backend.serverURL = urlField
        mosaic.attach(backend: backend.backend)
        isReloading = true
        await mosaic.refresh(from: backend.backend)
        isReloading = false
    }
}
