import SwiftUI

/// Bottom sheet that edits the page's frontmatter. Pushed from the
/// page's `⋯` button. Renders the YAML rows the canvas's T-X5
/// properties sheet had: type, slug, title, tags, status, created,
/// edited. Save flushes through the mosaic service.
struct PagePropertiesSheet: View {
    let page: Page
    @Binding var tags: [String]
    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    @State private var title: String
    @State private var slug: String
    @State private var status: String

    init(page: Page, tags: Binding<[String]>) {
        self.page = page
        self._tags = tags
        self._title = State(initialValue: page.title)
        self._slug = State(initialValue: page.slug)
        self._status = State(initialValue: "doing")
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("type") {
                        KindBadge(kind: page.type)
                    }
                    TextField("slug", text: $slug)
                        .font(.system(.body, design: .monospaced))
                        .textInputAutocapitalization(.never)
                    TextField("title", text: $title)
                } header: {
                    Text("Identity")
                } footer: {
                    Text("frontmatter · YAML on disk")
                        .font(.system(size: 10.5, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }

                Section("Tags") {
                    if tags.isEmpty {
                        Text("No tags").foregroundStyle(theme.fgFaint)
                    } else {
                        // Wrap chips in a flow layout via wrapping HStack.
                        FlowLayout(spacing: 6) {
                            ForEach(tags, id: \.self) { name in
                                HStack(spacing: 4) {
                                    TagChip(value: name)
                                    Button {
                                        tags.removeAll { $0.lowercased() == name.lowercased() }
                                    } label: {
                                        Image(systemName: "xmark.circle.fill")
                                            .font(.system(size: 12))
                                            .foregroundStyle(theme.fgFaint)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                    }
                }

                Section("Workflow") {
                    Picker("status", selection: $status) {
                        Text("todo").tag("todo")
                        Text("doing").tag("doing")
                        Text("done").tag("done")
                        Text("blocked").tag("blocked")
                    }
                }

                Section("History") {
                    LabeledContent("created", value: "2026-05-15 09:24")
                        .font(.system(.body, design: .monospaced))
                    LabeledContent("edited",  value: "2026-05-17 12:14")
                        .font(.system(.body, design: .monospaced))
                    LabeledContent("refs",    value: "\(page.refs) in")
                        .font(.system(.body, design: .monospaced))
                }

                Section {
                    Button(role: .destructive) {
                        dismiss()
                    } label: {
                        Label("Delete page", systemImage: "trash")
                    }
                }
            }
            .navigationTitle("Properties")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") { dismiss() }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") { dismiss() }
                }
            }
        }
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.visible)
    }
}

/// Tiny flow-wrapping layout for chip rows so multi-tag pages don't
/// overflow off-screen.
struct FlowLayout: Layout {
    var spacing: CGFloat = 8

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let width = proposal.width ?? .infinity
        var maxRowHeight: CGFloat = 0
        var totalHeight: CGFloat = 0
        var rowWidth: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if rowWidth + size.width > width {
                totalHeight += maxRowHeight + spacing
                rowWidth = 0
                maxRowHeight = 0
            }
            rowWidth += size.width + spacing
            maxRowHeight = max(maxRowHeight, size.height)
        }
        totalHeight += maxRowHeight
        return CGSize(width: width, height: totalHeight)
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        var x = bounds.minX
        var y = bounds.minY
        var rowHeight: CGFloat = 0
        for subview in subviews {
            let size = subview.sizeThatFits(.unspecified)
            if x + size.width > bounds.maxX {
                x = bounds.minX
                y += rowHeight + spacing
                rowHeight = 0
            }
            subview.place(at: CGPoint(x: x, y: y), proposal: ProposedViewSize(size))
            x += size.width + spacing
            rowHeight = max(rowHeight, size.height)
        }
    }
}
