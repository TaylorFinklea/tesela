import SwiftUI

/// Graphite primitives gallery — the iOS analogue of the web `/g` page.
/// A foundation visual-proof + dev harness: renders every Graphite
/// primitive on the Graphite surface. Wired up via `#Preview` (and
/// injectable as a dev screen) — replaced by the real shell at cutover.
struct GrGalleryView: View {
    @Environment(\.theme) private var theme

    private let types = ["task", "event", "note", "project", "person", "query"]

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 20) {
                Text("Graphite primitives")
                    .font(.system(size: 19, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)

                // Buttons
                HStack(spacing: 10) {
                    GrButton(variant: .cta, label: "New note")
                    GrButton(variant: .ghost, label: "Ghost")
                    GrButton(variant: .ghost, icon: "settings")
                }

                // Chips
                HStack(spacing: 8) {
                    GrChip(label: "Tasks", active: true, count: 12)
                    GrChip(label: "Notes", count: 4)
                }

                // Type dots
                HStack(spacing: 14) {
                    ForEach(types, id: \.self) { t in
                        HStack(spacing: 6) {
                            GrTypeDot(kind: t)
                            Text(t)
                                .font(.system(size: 10.5, design: .monospaced))
                                .foregroundStyle(theme.fgFaint)
                        }
                    }
                }

                // Type tags
                HStack(spacing: 8) {
                    GrTypeTag(kind: "project")
                    GrTypeTag(kind: "task")
                }

                // Widget + rows
                GrWidget(title: "Today", icon: "sun", badge: "3") {
                    GrRow(icon: "circle-dot", label: "Write the plan", meta: "2h")
                    GrRow(icon: "circle-dot", label: "Review PR", meta: "now", urgent: true)
                }
            }
            .padding(32)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .background(theme.bg.ignoresSafeArea())
    }
}

#Preview {
    GrGalleryView()
        .environment(\.theme, .graphite)
}
