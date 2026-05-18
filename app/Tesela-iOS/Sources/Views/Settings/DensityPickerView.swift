import SwiftUI

/// Density tier picker — Comfortable / Compact / Compact+. Settings →
/// Appearance → Density. Live preview row at the bottom of each option
/// so the user can see what the change does before committing.
struct DensityPickerView: View {
    @ObservedObject var appearance: AppearanceController

    @Environment(\.theme) private var theme

    var body: some View {
        Form {
            Section {
                ForEach(DensityTier.allCases) { tier in
                    Button {
                        appearance.density = tier
                    } label: {
                        densityRow(for: tier, active: appearance.density == tier)
                    }
                    .buttonStyle(.plain)
                }
            } header: {
                Text("Body text density")
            } footer: {
                Text("Mobile defaults to slightly less dense than desktop. Pick the tier that feels right under your thumb.")
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
        }
        .scrollContentBackground(.hidden)
        .background(theme.bg)
        .navigationTitle("Density")
        .navigationBarTitleDisplayMode(.inline)
    }

    private func densityRow(for tier: DensityTier, active: Bool) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(tier.displayName)
                    .foregroundStyle(theme.fgDefault)
                Spacer()
                if active {
                    Image(systemName: "checkmark")
                        .font(.system(size: 16, weight: .semibold))
                        .foregroundStyle(theme.accentPrimary)
                }
            }
            Text("\(Int(tier.bodySize))pt body · \(String(format: "%.2f", tier.lineHeight))× line-height")
                .font(.system(size: tier.bodySize, design: .default))
                .foregroundStyle(theme.fgMuted)
                .lineSpacing(2)
        }
        .padding(.vertical, 2)
    }
}
