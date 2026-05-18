import SwiftUI

/// A single outliner block — one `- ` bullet on the web. Renders the
/// bullet, the body text (with inline wiki/bold), the trailing tag
/// chips, and respects indent depth.
///
/// Three interactions:
/// 1. Tap the row → `onTap()` (consumers normally route this to "begin edit")
/// 2. Tap a task checkbox → `onToggleTask()`
/// 3. Long-press → `BlockContextMenu` via `.contextMenu`
///
/// When `isEditing` is true, the body renders as a `TextField` with
/// focus + an `onSubmit` that calls `onCommitEdit(newText)`. Owners
/// hold the editing-id state and pass `isEditing = (block.id == editingId)`.
struct BlockRow: View {
    let id: String
    let kind: BlockKind
    let text: String
    var indent: Int = 0
    var isDone: Bool = false
    var tags: [String] = []
    var isEditing: Bool = false

    var onToggleTask: (() -> Void)? = nil
    var onTap: (() -> Void)? = nil
    var onCommitEdit: ((String) -> Void)? = nil
    var onCancelEdit: (() -> Void)? = nil
    var onMenuAction: ((BlockAction) -> Void)? = nil

    @Environment(\.theme) private var theme
    @State private var editBuffer: String = ""
    @FocusState private var editFocused: Bool

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            bullet
            VStack(alignment: .leading, spacing: 4) {
                content
                if !tags.isEmpty && !isEditing {
                    HStack(spacing: 4) {
                        ForEach(tags, id: \.self) { tag in
                            TagChip(value: tag)
                        }
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, CGFloat(18 + indent * 18))
        .padding(.trailing, 18)
        .padding(.vertical, 6)
        .contentShape(Rectangle())
        .onTapGesture {
            handleTap()
        }
        .contextMenu {
            BlockContextMenu(blockId: id) { action in
                onMenuAction?(action)
            }
        }
    }

    private func handleTap() {
        // Tap on a task's row toggles done. Tap on a non-task row
        // enters edit mode if an onTap handler is wired up.
        if kind == .task {
            onToggleTask?()
        } else {
            onTap?()
        }
    }

    // ── Bullet (task checkbox or project dot or note dot) ───────────────

    @ViewBuilder
    private var bullet: some View {
        switch kind {
        case .task:
            taskCheckbox
        case .project:
            Text("·")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.typeProject)
                .frame(width: 14, alignment: .center)
                .padding(.top, 2)
        default:
            Text("·")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.fgFaint)
                .frame(width: 14, alignment: .center)
                .padding(.top, 2)
        }
    }

    private var taskCheckbox: some View {
        ZStack {
            RoundedRectangle(cornerRadius: 3)
                .stroke(theme.typeTask, lineWidth: 1.5)
                .frame(width: 14, height: 14)
            if isDone {
                RoundedRectangle(cornerRadius: 3)
                    .fill(theme.typeTask)
                    .frame(width: 14, height: 14)
                Icon(name: .check, size: 10, lineWidth: 2.5)
                    .foregroundStyle(theme.bg)
            }
        }
        .padding(.top, 4)
        .onTapGesture {
            if kind == .task { onToggleTask?() }
        }
    }

    // ── Content (body text with inline parsing OR a TextField) ──────────

    private var contentColor: Color {
        isDone ? theme.fgSubtle : theme.fgDefault
    }

    @ViewBuilder
    private var content: some View {
        if isEditing {
            editField
        } else {
            renderedText
        }
    }

    private var renderedText: some View {
        BlockText(text: text)
            .font(.system(size: 15))
            .foregroundStyle(contentColor)
            .strikethrough(isDone, color: theme.fgSubtle)
            .lineSpacing(3)
            .fixedSize(horizontal: false, vertical: true)
    }

    private var editField: some View {
        TextField("Block text", text: $editBuffer, axis: .vertical)
            .font(.system(size: 15))
            .foregroundStyle(theme.fgDefault)
            .tint(theme.accentPrimary)
            .focused($editFocused)
            .submitLabel(.done)
            .onAppear {
                editBuffer = text
                // Defer the focus so SwiftUI has time to lay out the
                // TextField before pulling the keyboard up.
                DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
                    editFocused = true
                }
            }
            .onSubmit {
                commitEdit()
            }
            .onChange(of: editFocused) { _, focused in
                // Blurring the field commits whatever's there. Mirrors
                // Apple Notes — taps elsewhere finalize the edit.
                if !focused && isEditing {
                    commitEdit()
                }
            }
    }

    private func commitEdit() {
        let trimmed = editBuffer.trimmingCharacters(in: .whitespacesAndNewlines)
        onCommitEdit?(trimmed)
    }
}
