import SwiftUI

/// Theme picker. Lists all 17 themes (Prism indigo default + 16 dark
/// variants) with a 3-swatch preview per theme. Tapping a row switches
/// the active theme via the AppearanceController, which repaints the
/// entire app.
struct ThemePickerView: View {
    @ObservedObject var appearance: AppearanceController

    @Environment(\.theme) private var theme

    var body: some View {
        Form {
            Section {
                ForEach(Theme.all) { t in
                    Button {
                        appearance.themeID = t.id
                    } label: {
                        themeRow(for: t, active: appearance.themeID == t.id)
                    }
                    .buttonStyle(.plain)
                }
            } header: {
                Text("All themes")
            } footer: {
                Text("Always dark on first ship. Light themes land in a later update.")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Theme")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func themeRow(for t: Theme, active: Bool) -> some View {
        HStack(spacing: 12) {
            // Swatch row — bg, accent-primary, accent-secondary
            HStack(spacing: 2) {
                RoundedRectangle(cornerRadius: 3)
                    .fill(t.bg)
                    .frame(width: 30, height: 22)
                    .overlay(RoundedRectangle(cornerRadius: 3).stroke(theme.line, lineWidth: 1))
                RoundedRectangle(cornerRadius: 3)
                    .fill(t.accentPrimary)
                    .frame(width: 12, height: 22)
                    .overlay(RoundedRectangle(cornerRadius: 3).stroke(theme.line, lineWidth: 1))
                RoundedRectangle(cornerRadius: 3)
                    .fill(t.accentSecondary)
                    .frame(width: 12, height: 22)
                    .overlay(RoundedRectangle(cornerRadius: 3).stroke(theme.line, lineWidth: 1))
            }

            VStack(alignment: .leading, spacing: 2) {
                HStack(spacing: 6) {
                    Text(t.id.displayName)
                        .foregroundStyle(theme.fgDefault)
                    if t.id == .prismIndigo {
                        Text("default")
                            .font(.system(size: 9, weight: .semibold, design: .monospaced))
                            .padding(.horizontal, 6)
                            .padding(.vertical, 1)
                            .foregroundStyle(theme.accentPrimary)
                            .background(theme.accentPrimary.opacity(0.14))
                            .clipShape(Capsule())
                    }
                }
                Text(#"data-theme="\#(t.id.rawValue)""#)
                    .font(.system(size: 10, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
            Spacer()
            if active {
                Image(systemName: "checkmark")
                    .font(.system(size: 16, weight: .semibold))
                    .foregroundStyle(theme.accentPrimary)
            }
        }
        .padding(.vertical, 2)
    }
}
