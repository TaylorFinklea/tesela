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
    /// Commit current text and append a new sibling block immediately
    /// after this one, then transfer focus to it. Wired by parents that
    /// own the outline (Daily, Page) so the keyboard accessory's Enter
    /// behaviour can split a block.
    var onSplitToNewBlock: ((String) -> Void)? = nil
    /// Apply an indent delta to this block (+1 or -1). Used by the
    /// keyboard accessory toolbar's indent/dedent buttons.
    var onIndent: ((Int) -> Void)? = nil
    /// Cycle the block's kind/status (note → open task → done → note).
    var onCycleStatus: (() -> Void)? = nil

    @Environment(\.theme) private var theme
    @State private var editBuffer: String = ""
    @FocusState private var editFocused: Bool

    @AppStorage("keyboardToolbarItems") private var keyboardToolbarRaw: String = defaultKeyboardToolbarItemsRaw

    private var configuredToolbarItems: [KeyboardToolbarItem] {
        decodeKeyboardToolbarItems(keyboardToolbarRaw)
    }

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
        // Tap anywhere on the row enters edit mode, regardless of kind.
        // Tap-to-toggle for tasks is handled by the checkbox's own
        // gesture so tapping the text body still lets you edit a task.
        onTap?()
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
                // When entering edit mode, inline the tags so the
                // user can edit them as raw `#tag` text alongside the
                // body. They're parsed back out in `commitEdit`.
                editBuffer = combinedEditableText()
                editFocused = true
            }
            .onSubmit { commitEdit() }
            .onChange(of: editBuffer) { _, newValue in
                // Detect "Enter on an empty line" by looking for a
                // trailing double-newline. Strip it from the current
                // block and ask the parent to split: commit this block
                // (without the trailing blank line) and append a new
                // empty block with focus.
                if newValue.hasSuffix("\n\n") {
                    let stripped = String(newValue.dropLast(2))
                    onSplitToNewBlock?(stripped.trimmingCharacters(in: .whitespacesAndNewlines))
                }
            }
            .onChange(of: editFocused) { _, focused in
                // Blurring the field commits whatever's there. Mirrors
                // Apple Notes — taps elsewhere finalize the edit.
                if !focused && isEditing {
                    commitEdit()
                }
            }
            .toolbar {
                if isEditing {
                    ToolbarItemGroup(placement: .keyboard) {
                        keyboardAccessory
                    }
                }
            }
    }

    @ViewBuilder
    private var keyboardAccessory: some View {
        HStack(spacing: 18) {
            ForEach(configuredToolbarItems) { item in
                toolbarButton(for: item)
            }
            Spacer()
        }
    }

    private func toolbarButton(for item: KeyboardToolbarItem) -> some View {
        Button {
            handleToolbarAction(item)
        } label: {
            Image(systemName: item.systemImage)
        }
        .accessibilityLabel(item.label)
    }

    private func handleToolbarAction(_ item: KeyboardToolbarItem) {
        switch item {
        case .hideKeyboard:
            editFocused = false
        case .slashCommand:
            if !editBuffer.hasSuffix("/") { editBuffer += "/" }
        case .tags:
            if !editBuffer.hasSuffix("#") {
                editBuffer += (editBuffer.hasSuffix(" ") || editBuffer.isEmpty ? "" : " ") + "#"
            }
        case .dedent:
            onIndent?(-1)
        case .indent:
            onIndent?(1)
        case .cycleStatus:
            onCycleStatus?()
        case .mic, .deadline, .schedule:
            // Stubs — these UI affordances surface in the toolbar so
            // users can opt in, but the actions land in later phases
            // (voice-into-block, due date picker, scheduled picker).
            break
        }
    }

    private func commitEdit() {
        let trimmed = editBuffer.trimmingCharacters(in: .whitespacesAndNewlines)
        onCommitEdit?(trimmed)
    }

    /// Body text + inline `#tags` so the user can edit tags as raw
    /// text in the same TextField. Tags are joined with a separating
    /// space; if the body is empty we just emit the tags.
    private func combinedEditableText() -> String {
        let normalized = tags.map { $0.hasPrefix("#") ? $0 : "#\($0)" }.joined(separator: " ")
        if normalized.isEmpty { return text }
        if text.isEmpty { return normalized }
        return text + " " + normalized
    }
}
