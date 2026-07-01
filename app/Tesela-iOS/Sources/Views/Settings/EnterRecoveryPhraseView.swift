import SwiftUI

/// Pair via 24-word recovery phrase. Replaces the old 6-character
/// short-code joiner path (`PairWithShortCodeView`, removed) — the
/// phrase encodes the group key directly, so recovery works even
/// when no other device is nearby to show a live short code.
///
/// The view calls `recoverPairingFromPhrase(relayUrl:phrase:)`, which
/// re-derives the group key from the phrase, discovers the group's
/// current pairing material on the given relay, and hands back a
/// relay-only pairing code. That code is then adopted exactly like a
/// relay-only QR scan in `PairScanView.adopt(_:)` — cache it, switch
/// to `.relay` mode, and fire `onPaired`.
struct EnterRecoveryPhraseView: View {
    @ObservedObject var backend: BackendSettings
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var registry: MosaicRegistry

    /// Success signal threaded down from `PairDeviceView`; see its doc
    /// comment. Fired in `submit()`, not touched otherwise.
    var onPaired: ((String?) -> Void)? = nil

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var phrase: String = ""
    @State private var relayUrl: String = "https://tesela-relay.finklea.workers.dev"
    @State private var working: Bool = false
    @State private var error: String?

    /// Whitespace/newline-separated, lowercased words. The FFI is the
    /// source of truth for real validation (wordlist membership,
    /// checksum); this is only used to gate the submit button on an
    /// obviously-incomplete phrase.
    private var normalizedWords: [String] {
        phrase
            .lowercased()
            .split(whereSeparator: { $0.isWhitespace })
            .map(String.init)
    }

    private var trimmedRelayUrl: String {
        relayUrl.trimmingCharacters(in: .whitespacesAndNewlines)
    }

    private var canSubmit: Bool {
        normalizedWords.count == 24 && !trimmedRelayUrl.isEmpty && !working
    }

    var body: some View {
        Form {
            Section {
                TextEditor(text: $phrase)
                    .font(.system(.body, design: .monospaced))
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .frame(minHeight: 120)
            } header: {
                Text("Recovery phrase")
            } footer: {
                Text("Enter the 24-word recovery phrase from another device, separated by spaces or newlines. (\(normalizedWords.count)/24 words)")
                    .font(.system(size: 11, design: .monospaced))
            }

            Section {
                TextField("https://tesela-relay.finklea.workers.dev", text: $relayUrl)
                    .font(.system(size: 13, design: .monospaced))
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
            } header: {
                Text("Relay")
            } footer: {
                Text("The relay where this mosaic syncs. Change it if your group uses a different relay.")
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
                        Text(working ? "Recovering…" : "Recover mosaic")
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
        .navigationTitle("Enter recovery phrase")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") { dismiss() }
                    .tint(theme.fgMuted)
            }
        }
    }

    // MARK: - Submit

    @MainActor
    private func submit() async {
        guard canSubmit else { return }
        working = true
        error = nil
        defer { working = false }

        let normalizedPhrase = normalizedWords.joined(separator: " ")

        do {
            let pairingCode = try await recoverPairingFromPhrase(
                relayUrl: trimmedRelayUrl,
                phrase: normalizedPhrase
            )
            adopt(pairingCode)
        } catch let err as FfiSyncError {
            error = err.localizedDescription
        } catch {
            self.error = (error as NSError).localizedDescription
        }
    }

    // MARK: - Adopt (mirrors `PairScanView.adopt`'s relay-only branch)

    /// `recoverPairingFromPhrase` always returns a RELAY-only pairing
    /// code (the phrase alone can't hand us a reachable HTTP `url`),
    /// so this always takes the relay branch of
    /// `PairScanView.adopt(_:)`: cache the code, switch to `.relay`
    /// mode, fire `onPaired`, dismiss.
    private func adopt(_ pairingCode: String) {
        RelayTicker.cachePairingCode(pairingCode)
        backend.mode = .relay
        let displayName = (try? decodePairingCode(code: pairingCode))?.displayName ?? "Recovered mosaic"
        onPaired?(displayName)
        dismiss()
    }
}
