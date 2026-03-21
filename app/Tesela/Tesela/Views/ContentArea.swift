import SwiftUI

// MARK: - ContentArea
// Center pane: shows the current page in the outliner editor

struct ContentArea: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        Group {
            if let page = appState.currentPage {
                PageEditorView(page: page)
            } else {
                EmptyStateView()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

// MARK: - EmptyStateView
private struct EmptyStateView: View {
    @Environment(AppState.self) private var appState

    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "doc.text")
                .font(.system(size: 48))
                .foregroundStyle(.tertiary)
            Text("No page selected")
                .font(.title3)
                .foregroundStyle(.secondary)
            Text("Select a page from the sidebar or press ⌘K to search")
                .font(.caption)
                .foregroundStyle(.tertiary)
                .multilineTextAlignment(.center)
            Button("Open Today's Journal") {
                Task {
                    if let page = try? await appState.api.getDailyNote() {
                        appState.open(page)
                    }
                }
            }
            .buttonStyle(.bordered)
            .padding(.top, 8)
        }
        .padding()
    }
}

// MARK: - PageEditorView
// Editable text view with 500ms debounced auto-save.
// Phase 11.3/11.4 will replace the TextEditor with the full block OutlinerView.
struct PageEditorView: View {
    let page: Page
    @Environment(AppState.self) private var appState
    @State private var editedBody: String = ""
    @State private var saveTask: Task<Void, Never>?
    @State private var showDeleteConfirm = false

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Title + toolbar
            HStack {
                Text(page.title)
                    .font(.title2)
                    .bold()
                    .lineLimit(1)
                Spacer()
                Text(page.modifiedAt, style: .relative)
                    .font(.caption)
                    .foregroundStyle(.tertiary)
                Button(role: .destructive) {
                    showDeleteConfirm = true
                } label: {
                    Image(systemName: "trash")
                }
                .buttonStyle(.borderless)
                .help("Delete page")
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 12)

            Divider()

            TextEditor(text: $editedBody)
                .font(.system(.body, design: .monospaced))
                .padding(24)
                .onChange(of: editedBody) { _, _ in
                    scheduleAutoSave()
                }
        }
        .onAppear {
            editedBody = page.body
        }
        .onChange(of: page.id) { _, _ in
            // Navigated to a different page — flush pending save and load new body
            saveTask?.cancel()
            editedBody = page.body
        }
        .alert("Delete \"\(page.title)\"?", isPresented: $showDeleteConfirm) {
            Button("Delete", role: .destructive) {
                Task { await appState.deletePage(page) }
            }
            Button("Cancel", role: .cancel) {}
        } message: {
            Text("This permanently deletes the page and its file. This cannot be undone.")
        }
    }

    private func scheduleAutoSave() {
        saveTask?.cancel()
        saveTask = Task {
            try? await Task.sleep(for: .milliseconds(500))
            guard !Task.isCancelled else { return }
            await appState.updatePage(id: page.id, newBody: editedBody)
        }
    }
}
