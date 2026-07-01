import SwiftUI

/// Show this device's 24-word recovery phrase, derived from the
/// cached pairing code via `recoveryPhraseFromPairingCode(code:)` —
/// the inverse of `EnterRecoveryPhraseView`'s
/// `recoverPairingFromPhrase(relayUrl:phrase:)`.
///
/// The words are hidden behind an explicit reveal tap (Anytype-style
/// warning first) — this is the one screen in the app that displays
/// key material in the clear.
struct ShowRecoveryPhraseView: View {
    @Environment(\.theme) private var theme

    @State private var words: [String]?
    @State private var loadError: String?
    @State private var revealed: Bool = false

    var body: some View {
        Form {
            if let words {
                if revealed {
                    phraseSection(words)
                } else {
                    revealGateSection
                }
            } else if let loadError {
                Section {
                    Text(loadError)
                        .font(.system(size: 12, design: .monospaced))
                        .foregroundStyle(theme.fgMuted)
                }
            } else {
                emptyStateSection
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Recovery phrase")
        .navigationBarTitleDisplayMode(.inline)
        .onAppear(perform: load)
    }

    // MARK: - Load

    private func load() {
        guard words == nil, loadError == nil else { return }
        guard let code = RelayTicker.cachedPairingCode() else {
            loadError = "This device isn't paired yet. Pair with another device first — then its recovery phrase will show here."
            return
        }
        do {
            let phrase = try recoveryPhraseFromPairingCode(code: code)
            words = phrase.split(whereSeparator: { $0.isWhitespace }).map(String.init)
        } catch let err as FfiSyncError {
            loadError = err.localizedDescription
        } catch {
            loadError = (error as NSError).localizedDescription
        }
    }

    // MARK: - Empty state

    private var emptyStateSection: some View {
        Section {
            HStack(spacing: 10) {
                ProgressView()
                Text("Loading…")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgMuted)
            }
        }
    }

    // MARK: - Reveal gate

    private var revealGateSection: some View {
        Section {
            VStack(alignment: .leading, spacing: 12) {
                Text("Write these down and keep them safe. Anyone with this phrase can read your mosaic — and we can't recover them for you.")
                    .font(.system(size: 13))
                    .foregroundStyle(theme.typeTask)
                    .fixedSize(horizontal: false, vertical: true)

                Button {
                    revealed = true
                } label: {
                    Text("Reveal recovery phrase")
                        .font(.system(size: 14, weight: .semibold))
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, 10)
                        .foregroundStyle(theme.bg)
                        .background(theme.accentPrimary)
                        .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
            }
            .padding(.vertical, 6)
        }
    }

    // MARK: - Revealed phrase

    private func phraseSection(_ words: [String]) -> some View {
        Section {
            Text("Anyone with this phrase can read your mosaic. Keep it private.")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.typeTask)

            LazyVGrid(columns: [GridItem(.flexible()), GridItem(.flexible())], spacing: 10) {
                ForEach(Array(words.enumerated()), id: \.offset) { index, word in
                    HStack(spacing: 6) {
                        Text("\(index + 1).")
                            .font(.system(size: 12, design: .monospaced))
                            .foregroundStyle(theme.fgFaint)
                            .frame(width: 20, alignment: .trailing)
                        Text(word)
                            .font(.system(size: 13, design: .monospaced))
                            .foregroundStyle(theme.fgDefault)
                    }
                }
            }
            .padding(.vertical, 4)
        } header: {
            Text("24-word phrase")
        }
    }
}
