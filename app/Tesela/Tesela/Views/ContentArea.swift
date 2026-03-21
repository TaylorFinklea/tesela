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
// Placeholder — Phase 11.3/11.4 will replace this with the full OutlinerView
struct PageEditorView: View {
    let page: Page
    @Environment(AppState.self) private var appState

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // Page title bar
            HStack {
                Text(page.title)
                    .font(.title2)
                    .bold()
                Spacer()
                Text(page.modifiedAt, style: .relative)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
            .padding(.horizontal, 24)
            .padding(.vertical, 12)

            Divider()

            // Raw body — placeholder until OutlinerView is built
            ScrollView {
                Text(page.body)
                    .font(.system(.body, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(24)
                    .textSelection(.enabled)
            }
        }
    }
}
