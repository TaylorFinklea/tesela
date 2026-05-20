import SwiftUI

/// Pair via 6-character short code. The user types the code shown on
/// the inviter's pairing screen; the view calls the inviter's
/// `GET /sync/peer/short-code/:code` to resolve the short code to the
/// full base64url pairing payload, then runs the same "adopt URL" path
/// as the QR scanner.
///
/// Today this still goes through the iOS app's already-configured
/// backend URL (`backend.serverURL`) because that's the only server
/// the phone knows about. When iOS gets multi-server pairing the URL
/// becomes a separate field on this view.
struct PairWithShortCodeView: View {
    @ObservedObject var backend: BackendSettings
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var registry: MosaicRegistry

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var code: String = ""
    @State private var working: Bool = false
    @State private var error: String?
    @State private var resolved: PairingCodeRecord?

    private var trimmed: String {
        code.uppercased().filter { $0.isLetter || $0.isNumber }
    }

    private var canSubmit: Bool {
        trimmed.count == 6 && !working
    }

    var body: some View {
        Form {
            Section {
                TextField("ABC123", text: $code)
                    .font(.system(.title2, design: .monospaced))
                    .textInputAutocapitalization(.characters)
                    .autocorrectionDisabled()
                    .keyboardType(.asciiCapable)
                    .submitLabel(.go)
                    .onSubmit { Task { await submit() } }
            } header: {
                Text("Short code")
            } footer: {
                Text("Type the 6 characters shown under the QR on the inviter's pairing screen. Codes expire ~10 minutes after they're generated.")
                    .font(.system(size: 11, design: .monospaced))
            }

            if let error {
                Section {
                    Text(error)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(theme.typeTask)
                }
            }

            Section {
                Button {
                    Task { await submit() }
                } label: {
                    HStack {
                        if working {
                            ProgressView()
                                .tint(theme.bg)
                        }
                        Text(working ? "Looking up…" : "Pair via code")
                            .font(.system(size: 14, weight: .semibold))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 8)
                    .foregroundStyle(canSubmit ? theme.bg : theme.fgFaint)
                    .background(canSubmit ? theme.accentPrimary : theme.bg3)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
                .disabled(!canSubmit)
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Pair via code")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") { dismiss() }
                    .tint(theme.fgMuted)
            }
        }
        .sheet(item: $resolved) { record in
            confirmSheet(for: record)
        }
    }

    // MARK: - Submit / resolve

    @MainActor
    private func submit() async {
        guard canSubmit else { return }
        working = true
        error = nil
        defer { working = false }

        guard case .http(let baseURL) = backend.backend else {
            error = "Set the backend URL in Settings → Backend first; the short code is looked up against that server."
            return
        }
        let url = baseURL.appending(path: "sync/peer/short-code").appending(path: trimmed)
        var req = URLRequest(url: url)
        req.cachePolicy = .reloadIgnoringLocalCacheData
        do {
            let (data, resp) = try await URLSession.shared.data(for: req)
            guard let http = resp as? HTTPURLResponse else {
                error = "Bad response from server"
                return
            }
            if http.statusCode == 404 {
                error = "Short code expired or unknown. Ask the other device to regenerate."
                return
            }
            if !(200..<300).contains(http.statusCode) {
                error = "HTTP \(http.statusCode): \(String(data: data, encoding: .utf8) ?? "unknown")"
                return
            }
            let decoded = try JSONDecoder().decode(ShortCodeLookupResponse.self, from: data)
            // Validate the embedded long code is actually well-formed.
            let record = try decodePairingCode(code: decoded.code)
            resolved = record
        } catch let e as DecodingError {
            error = "Couldn't read server response: \(e.localizedDescription)"
        } catch {
            self.error = (error as NSError).localizedDescription
        }
    }

    // MARK: - Confirmation sheet

    private func confirmSheet(for record: PairingCodeRecord) -> some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("Inviter") {
                        Text(record.displayName)
                            .font(.system(.body, design: .monospaced))
                    }
                    LabeledContent("URL") {
                        Text(record.url)
                            .font(.system(size: 12, design: .monospaced))
                            .lineLimit(2)
                    }
                } header: {
                    Text("Pair with this device")
                } footer: {
                    Text("Saving switches this iPhone's backend to the inviter's server.")
                        .font(.system(size: 11, design: .monospaced))
                }

                Section {
                    Button {
                        adopt(record)
                    } label: {
                        Text("Pair & connect")
                            .font(.system(size: 14, weight: .semibold))
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    Button("Cancel") { resolved = nil }
                        .tint(.secondary)
                }
            }
            .navigationTitle("Confirm pair")
            .navigationBarTitleDisplayMode(.inline)
        }
        .presentationDetents([.medium])
    }

    private func adopt(_ record: PairingCodeRecord) {
        backend.mode = .http
        backend.serverURL = record.url
        Task {
            // Pairing handoff: import the inviter server's mosaics and
            // activate its current one; AppShell loads from there.
            await registry.importDiscovered(serverURL: record.url, activateCurrent: true)
            if registry.activeProfile == nil {
                mosaic.attach(backend: backend.backend)
                await mosaic.refresh(from: backend.backend)
            }
        }
        resolved = nil
        dismiss()
    }
}

/// Mirror of the server's `PairingCodePayload`. We only need `code`
/// (the long base64url string) — the rest comes back from
/// `decodePairingCode` once we have it.
private struct ShortCodeLookupResponse: Decodable {
    let code: String
}
