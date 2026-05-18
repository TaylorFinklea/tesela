import SwiftUI

/// Voice (Parakeet v3) settings. Top-level under Settings per decision
/// #12 — moved out of Bridges since voice is a Tesela-internal feature,
/// not a bridge to another app.
///
/// Model integration ships in a later phase; this is the configuration
/// surface (toggles + language picker + status row).
struct VoiceSettingsView: View {
    @AppStorage("voice.autoPunctuation") private var autoPunctuation = true
    @AppStorage("voice.splitOnPauses") private var splitOnPauses = false
    @AppStorage("voice.language") private var language = "en-US"

    @Environment(\.theme) private var theme

    var body: some View {
        Form {
            Section {
                LabeledContent {
                    HStack(spacing: 6) {
                        Circle().fill(theme.typeQuery).frame(width: 6, height: 6)
                        Text("ready")
                            .foregroundStyle(theme.typeQuery)
                    }
                } label: {
                    VStack(alignment: .leading) {
                        Text("Parakeet v3")
                        Text("NVIDIA NeMo · 600M params · 0.6 GB")
                            .font(.system(size: 11, design: .monospaced))
                            .foregroundStyle(theme.fgFaint)
                    }
                }
                Picker("Language", selection: $language) {
                    Text("English (US)").tag("en-US")
                    Text("English (UK)").tag("en-GB")
                    Text("Spanish").tag("es-ES")
                    Text("French").tag("fr-FR")
                }
            } header: {
                Text("Model")
            }

            Section {
                Toggle("Auto-punctuation", isOn: $autoPunctuation)
                Toggle("Split on long pauses", isOn: $splitOnPauses)
            } header: {
                Text("Behavior")
            } footer: {
                Text("Auto-punctuation is heuristic only — no model call. Split on pauses adds a new block whenever you pause for over 1.5s.")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }

            Section {
                Text("Voice recording lands as text on today's daily by default. Use the mic button in the capture sheet to record.")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(theme.fgMuted)
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Voice")
        .navigationBarTitleDisplayMode(.inline)
    }
}
