import SwiftUI

/// Safari-tabs-style overlay showing every open page as a card.
/// Triggered from the page stack button on PageView's top bar. The
/// "swipe up from bottom edge" gesture lands in a later polish phase
/// — this is the explicit-trigger version.
struct OpenPagesOverlay: View {
    @ObservedObject var stack: PageStack
    @Binding var isPresented: Bool
    /// Called when the user picks a card to jump to.
    var onJump: (Page) -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 14) {
                ForEach(stack.openPages) { page in
                    pageCard(for: page)
                }
                if stack.openPages.isEmpty {
                    Text("No open pages")
                        .font(.system(size: 13, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                        .frame(maxWidth: .infinity, minHeight: 200)
                }
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 12)
        }
        .background(theme.bg)
        .presentationDetents([.height(280), .large])
        .presentationDragIndicator(.visible)
    }

    private func pageCard(for page: Page) -> some View {
        Button {
            onJump(page)
            isPresented = false
        } label: {
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    KindBadge(kind: page.type)
                    Spacer()
                    Button {
                        stack.close(page.id)
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 18))
                            .foregroundStyle(theme.fgFaint)
                    }
                    .buttonStyle(.plain)
                }
                Text(page.title)
                    .font(.system(size: 14, weight: .semibold))
                    .foregroundStyle(theme.fgDefault)
                    .multilineTextAlignment(.leading)
                Text("notes/\(page.slug).md")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                Spacer(minLength: 0)
                Text("\(page.blocks) blocks · edited \(page.edited)")
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }
            .padding(14)
            .frame(width: 200, height: 240, alignment: .topLeading)
            .background(theme.bg2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(theme.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }
}
