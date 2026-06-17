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
    var properties: [BlockProperty] = []
    var isEditing: Bool = false
    var isFoldable: Bool = false
    var isCollapsed: Bool = false

    var onToggleFold: (() -> Void)? = nil
    var onToggleTask: (() -> Void)? = nil
    var onTap: (() -> Void)? = nil
    var onCommitEdit: ((String) -> Void)? = nil
    /// Debounced live text updates while editing — distinct from
    /// `onCommitEdit`, which fires once when the edit finishes. Owners
    /// route this to a writeback so other devices see typing in
    /// progress without waiting for the block to be committed.
    var onTextChanged: ((String) -> Void)? = nil
    /// One local keystroke as a UTF-16 character splice (collab editing
    /// C1 outbound): delete `utf16DeleteLen` code units at `utf16Offset`,
    /// then insert `insert`. When wired, the editor uses `CollabTextView`
    /// and routes typing through this seam (→ engine `spliceBlockText`)
    /// instead of the whole-text `onTextChanged` re-author, so a peer's
    /// concurrent same-block edit is no longer clobbered. Owners that
    /// don't wire it fall back to the legacy `TextField`/`onTextChanged`
    /// path unchanged.
    var onTextSplice: ((_ utf16Offset: Int, _ utf16DeleteLen: Int, _ insert: String) -> Void)? = nil
    /// Collab editing C1-inbound: hand the owner this row's
    /// `CollabTextInserter` when the splice editor opens, so an inbound
    /// remote splice on THIS block can be live-applied to the live
    /// `UITextView` (caret remap) instead of waiting for the blur refresh.
    /// Fired with the inserter on the editor's `onAppear`. The owner is the
    /// gatekeeper (it reconciles only the block matching its `editingBlockId`),
    /// so no unregister-on-blur is needed — a stale inserter no-ops (its text
    /// view is held weakly).
    var onActiveCollabInserter: ((CollabTextInserter) -> Void)? = nil
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
    /// Persist an updated property list for this block. Called after the
    /// date sheet commits — the caller (DailyView/PageView) routes this
    /// to the appropriate service method.
    var onSetProperties: (([BlockProperty]) -> Void)? = nil
    /// Skip the current recurring-block occurrence to its next date.
    var onSkipRecurrence: (() -> Void)? = nil

    /// The `recurring::` property value, or `nil` if absent.
    private var recurringValue: String? {
        properties.first(where: { $0.key == "recurring" })?.value
    }

    /// The `deadline::` property value, or `nil` if absent.
    private var deadlineValue: String? {
        properties.first(where: { $0.key == "deadline" })?.value
    }

    /// The `scheduled::` property value, or `nil` if absent.
    private var scheduledValue: String? {
        properties.first(where: { $0.key == "scheduled" })?.value
    }

    // ── Task status + priority (web parity) ─────────────────────────────
    // Mirrors the web client so a task looks the same across web + mobile:
    // a status GLYPH (shape + color by status) beside the bullet — NOT a
    // checkbox — plus a priority FLAG in the sub-row. Web rules:
    // BlockOutliner.svelte statusChar/statusColorClass (857-873) + priority.ts.
    // The status/priority colors are fixed Tailwind/brand hexes on web (theme-
    // independent), so we match them as `Color(hex:)` constants, not theme tokens.

    /// `status::` value (lowercased), or nil when unset.
    private var statusValue: String? {
        let v = properties.first(where: { $0.key.lowercased() == "status" })?
            .value.trimmingCharacters(in: .whitespaces).lowercased()
        return (v?.isEmpty == false) ? v : nil
    }

    /// Status glyph — web statusChar(): a Task with status unset shows the
    /// `○` placeholder (web's shouldShowStatus placeholder).
    private var statusGlyph: String {
        switch statusValue ?? "" {
        case "done", "completed": return "✓"
        case "doing", "in-review": return "◑"
        case "todo": return "○"
        case "canceled", "cancelled": return "✗"
        case "blocked": return "⧖"
        case "paused": return "⏸"
        case "": return "○"
        default: return "·"
        }
    }

    /// Status color — web statusColorClass(), same fixed Tailwind hexes.
    private var statusGlyphColor: Color {
        switch statusValue ?? "" {
        case "done", "completed": return Color(hex: 0x34D399)            // emerald-400
        case "doing", "in-review": return Color(hex: 0x60A5FA)           // blue-400
        case "todo": return Color(hex: 0xFBBF24)                          // amber-400
        case "canceled", "cancelled", "blocked": return Color(hex: 0xF87171) // red-400
        case "paused": return theme.fgSubtle
        default: return theme.fgFaint
        }
    }

    /// Priority level 1–3 from `priority::` (web priority.ts); p4 / low /
    /// none / unset → nil → no flag (matches web).
    private var priorityLevel: Int? {
        guard let v = properties.first(where: { $0.key.lowercased() == "priority" })?
            .value.trimmingCharacters(in: .whitespaces).lowercased(), !v.isEmpty
        else { return nil }
        switch v {
        case "p1", "critical", "urgent", "1": return 1
        case "p2", "high", "2": return 2
        case "p3", "medium", "med", "3": return 3
        default: return nil
        }
    }

    /// Priority flag color — web priority.ts FLAGS (P1 red, P2 amber, P3 blue).
    private func priorityColor(_ level: Int) -> Color {
        switch level {
        case 1: return Color(hex: 0xEB5C58)
        case 2: return Color(hex: 0xE8A33D)
        default: return Color(hex: 0x6B9AE0)
        }
    }

    /// Block properties to render as right-edge chips — everything except the
    /// system/collection keys, the date/recurrence props that already get
    /// dedicated chips, and internal keys. Mirrors the web's hidden-key sets
    /// (`SYSTEM_HIDDEN_KEYS` + `ROW_OWNED_KEYS`); shows the rest (custom props
    /// like `points`/`testpoints`) so they're visible on iOS, not just desktop.
    private var displayProperties: [BlockProperty] {
        let hidden: Set<String> = [
            "query", "view", "views", "active_view", "collection",
            "scheduled", "deadline", "recurring", "status", "priority",
            "id", "collapsed", "color",
        ]
        return properties.filter {
            !hidden.contains($0.key.lowercased())
                && !$0.value.trimmingCharacters(in: .whitespaces).isEmpty
        }
    }

    @Environment(\.theme) private var theme
    @State private var editBuffer: String = ""
    @State private var livePushTask: Task<Void, Never>? = nil
    @FocusState private var editFocused: Bool
    /// Drives the `CollabTextView`'s first-responder state (the
    /// `UITextView`-backed splice editor replaces `@FocusState` for the
    /// collab path). Set true on appear; the coordinator flips it false
    /// on blur, which triggers the same commit the legacy editor did.
    @State private var collabFocused: Bool = false
    /// Imperative seam so the keyboard toolbar's text-inserting buttons
    /// insert at the live caret through the splice path. Recreated per
    /// row; bound to the concrete `UITextView` in `CollabTextView`.
    @State private var inserter = CollabTextInserter()

    @AppStorage("keyboardToolbarItems") private var keyboardToolbarRaw: String = defaultKeyboardToolbarItemsRaw
    @AppStorage("bareDateField") private var bareDateFieldRaw: String = "scheduled"
    @State private var showingDateSheet = false

    private var configuredToolbarItems: [KeyboardToolbarItem] {
        decodeKeyboardToolbarItems(keyboardToolbarRaw)
    }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            bullet
            VStack(alignment: .leading, spacing: 4) {
                content
                if (!tags.isEmpty || priorityLevel != nil || recurringValue != nil || deadlineValue != nil || scheduledValue != nil || !displayProperties.isEmpty) && !isEditing {
                    HStack(spacing: 4) {
                        // Priority flag (web parity: ⚑ + P1/P2/P3, priority-colored,
                        // in the sub-row — NOT tinting the marker). P4/low/unset → hidden.
                        if let level = priorityLevel {
                            HStack(spacing: 2) {
                                Text("⚑").font(.system(size: 10))
                                Text("P\(level)").font(.system(size: 11, weight: .semibold))
                            }
                            .foregroundStyle(priorityColor(level))
                        }
                        ForEach(tags, id: \.self) { tag in
                            TagChip(value: tag)
                        }
                        if let scheduledValue {
                            Button { showingDateSheet = true } label: {
                                ScheduledChip(value: scheduledValue)
                            }
                            .buttonStyle(.plain)
                        }
                        if let deadlineValue {
                            Button { showingDateSheet = true } label: {
                                DeadlineChip(value: deadlineValue)
                            }
                            .buttonStyle(.plain)
                        }
                        if let recValue = recurringValue {
                            Button { showingDateSheet = true } label: {
                                RecurrenceChip(value: recValue)
                            }
                            .buttonStyle(.plain)
                        }
                        ForEach(displayProperties, id: \.key) { prop in
                            PropertyChip(key: prop.key, value: prop.value)
                        }
                    }
                }
            }
            Spacer(minLength: 0)
        }
        .padding(.leading, CGFloat(18 + indent * 18))
        .padding(.trailing, 18)
        .padding(.vertical, 6)
        .overlay(alignment: .topLeading) {
            foldToggle
        }
        .contentShape(Rectangle())
        .onTapGesture {
            handleTap()
        }
        .contextMenu {
            BlockContextMenu(blockId: id) { action in
                onMenuAction?(action)
            }
        }
        .sheet(isPresented: $showingDateSheet) {
            DateInputSheet(
                initialScheduled: scheduledValue,
                initialDeadline: deadlineValue,
                initialRecurrence: recurringValue,
                canSkip: recurringValue != nil,
                bareDateFieldDefault: bareDateFieldRaw,
                onCommit: { field, iso, time, recurrence in
                    commitDate(field: field, iso: iso, time: time, recurrence: recurrence)
                    showingDateSheet = false
                },
                onSkip: {
                    onSkipRecurrence?()
                    showingDateSheet = false
                },
                onCancel: { showingDateSheet = false }
            )
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
    private var foldToggle: some View {
        if isFoldable {
            Button {
                onToggleFold?()
            } label: {
                Image(systemName: isCollapsed ? "chevron.right" : "chevron.down")
                    .font(.system(size: 10, weight: .semibold))
                    .foregroundStyle(isCollapsed ? theme.accentPrimary : theme.fgFaint)
                    .frame(width: 18, height: 24)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(isCollapsed ? "Expand block" : "Collapse block")
            .padding(.leading, CGFloat(indent * 18))
            .padding(.top, 4)
        }
    }

    @ViewBuilder
    private var bullet: some View {
        switch kind {
        case .task:
            // Web parity: a neutral bullet PLUS a colored status glyph beside
            // it (web keeps the dot and adds the status indicator — there is
            // NO checkbox). Tap the glyph to toggle done.
            HStack(alignment: .top, spacing: 4) {
                Text("·")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
                    .frame(width: 6, alignment: .center)
                statusMarker
            }
            .padding(.top, 2)
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

    /// The task status marker — web's status glyph (shape + color by status),
    /// replacing the old binary checkbox so a task looks the same as on web
    /// and reflects todo/doing/done/blocked/… not just done. Tap toggles done
    /// (v1 parity; web's full status cycle is a follow-up). Unset status on a
    /// task shows the dimmed `○` placeholder.
    private var statusMarker: some View {
        Text(statusGlyph)
            .font(.system(size: 12, design: .monospaced))
            .foregroundStyle(statusGlyphColor)
            .opacity(statusValue == nil ? 0.5 : 0.9)
            .frame(width: 14, alignment: .center)
            .contentShape(Rectangle())
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

    @ViewBuilder
    private var editField: some View {
        if onTextSplice != nil {
            collabEditField
        } else {
            legacyEditField
        }
    }

    /// Collab editing C1 outbound: a `UITextView`-backed editor that
    /// emits character splices on each keystroke (→ engine
    /// `spliceBlockText`) instead of re-authoring the whole block. This
    /// is what stops a peer's concurrent same-block edit from being
    /// clobbered. Used when the owner wires `onTextSplice` (today's
    /// daily). `editBuffer` is loaded as the ENGINE-EXACT block text
    /// (body + inline tags, see `combinedEditableText`) so splice offsets
    /// land correctly on the engine's `text_seq`. The keyboard accessory
    /// is passed as a hosted `inputAccessoryView` — NOT via `.toolbar
    /// { ToolbarItemGroup(placement: .keyboard) }`, which only attaches
    /// to SwiftUI-managed text inputs and silently shows nothing when a
    /// raw `UITextView` is the first responder — with its text-inserting
    /// buttons routed through `inserter` (the splice path) so they don't
    /// desync.
    private var collabEditField: some View {
        CollabTextView(
            text: $editBuffer,
            isFocused: $collabFocused,
            textColor: theme.fgDefault,
            tintColor: theme.accentPrimary,
            onSplice: { offset, deleteLen, insert in
                onTextSplice?(offset, deleteLen, insert)
            },
            onCommit: { final in
                commitEditCollab(final)
            },
            onSplitToNewBlock: { stripped in
                onSplitToNewBlock?(stripped)
            },
            inserter: inserter,
            accessory: collabKeyboardAccessory
        )
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear {
            editBuffer = combinedEditableText()
            collabFocused = true
            // Register this editor's imperative inserter so the owner can
            // live-apply an inbound remote splice on THIS block (C1-inbound).
            onActiveCollabInserter?(inserter)
        }
    }

    /// The collab editor's keyboard accessory, styled as a floating pill
    /// to match the system bar the legacy `TextField` path gets from
    /// `ToolbarItemGroup(placement: .keyboard)`. Hosted by
    /// `CollabTextView` as the `UITextView`'s `inputAccessoryView`
    /// (separate UIKit hierarchy), so theme + tint must be re-applied
    /// explicitly — the SwiftUI environment doesn't flow across. Vertical
    /// metrics must total `CollabTextView.accessoryBarHeight`.
    private var collabKeyboardAccessory: AnyView {
        AnyView(
            keyboardAccessory
                .padding(.horizontal, 16)
                .frame(height: 44)
                .glassEffect()
                .padding(.horizontal, 12)
                .padding(.top, 2)
                .padding(.bottom, 8)
                .tint(theme.accentPrimary)
                .environment(\.theme, theme)
        )
    }

    private var legacyEditField: some View {
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
                    livePushTask?.cancel()
                    onSplitToNewBlock?(stripped.trimmingCharacters(in: .whitespacesAndNewlines))
                    return
                }
                // Debounced live writeback (500ms, matching the web
                // client) so other devices see typing in progress
                // without waiting for the block to be committed.
                livePushTask?.cancel()
                let snapshot = newValue
                livePushTask = Task { @MainActor in
                    try? await Task.sleep(nanoseconds: 500_000_000)
                    guard !Task.isCancelled else { return }
                    onTextChanged?(snapshot)
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
        HStack(spacing: 12) {
            // Scrollable middle — user-configurable items. If the user
            // enables more buttons than fit, this scrolls horizontally
            // so the toolbar pill stays a normal width and the pinned
            // Hide-keyboard button on the right is always reachable.
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: 18) {
                    ForEach(scrollableToolbarItems) { item in
                        toolbarButton(for: item)
                    }
                }
                .padding(.horizontal, 2)
            }
            // Always pinned right — never scrolls, never configurable.
            toolbarButton(for: .hideKeyboard)
        }
    }

    /// Items rendered inside the scrollable middle. We filter out
    /// `.hideKeyboard` defensively — even if a legacy preference still
    /// has it in the stored list, it shouldn't double-render with the
    /// pinned trailing button.
    private var scrollableToolbarItems: [KeyboardToolbarItem] {
        configuredToolbarItems.filter { $0 != .hideKeyboard }
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
        // On the collab (UITextView) path, text-inserting buttons go
        // through the splice seam at the live caret so the editor and the
        // engine's `text_seq` stay aligned. The legacy `TextField` path
        // (no `onTextSplice`) keeps appending to `editBuffer`.
        let collab = onTextSplice != nil
        switch item {
        case .hideKeyboard:
            if collab { collabFocused = false } else { editFocused = false }
        case .slashCommand:
            if collab {
                inserter.insertAtCaret("/")
            } else if !editBuffer.hasSuffix("/") {
                editBuffer += "/"
            }
        case .backlink:
            // Insert an empty wikilink so the user types straight into
            // the link target. On collab, insert at the caret via the
            // splice path; on the legacy TextField (no cursor offset)
            // append at the end — caret lands there on next keystroke.
            if collab {
                inserter.insertAtCaret("[[]]")
            } else {
                let spacer = (editBuffer.hasSuffix(" ") || editBuffer.isEmpty) ? "" : " "
                editBuffer += spacer + "[[]]"
            }
        case .tags:
            if collab {
                inserter.insertAtCaret("#")
            } else if !editBuffer.hasSuffix("#") {
                editBuffer += (editBuffer.hasSuffix(" ") || editBuffer.isEmpty ? "" : " ") + "#"
            }
        case .dedent:
            onIndent?(-1)
        case .indent:
            onIndent?(1)
        case .cycleStatus:
            onCycleStatus?()
        case .date:
            showingDateSheet = true
        case .mic:
            // Stub — voice-into-block lands in a later phase.
            break
        }
    }

    private func commitEdit() {
        // The commit is the final word — drop any pending debounced
        // live push so it can't land after (and revert) the commit.
        livePushTask?.cancel()
        let trimmed = editBuffer.trimmingCharacters(in: .whitespacesAndNewlines)
        onCommitEdit?(trimmed)
    }

    /// Commit for the collab (splice) path. The block's text was already
    /// persisted keystroke-by-keystroke via splices, so this does NOT
    /// re-author the whole text — it just finalizes the edit (clears the
    /// editing state via `onCommitEdit`). Re-running a whole-text
    /// writeback here would Myers-diff against the engine and could
    /// re-clobber a peer's concurrent chars that arrived mid-edit, which
    /// is exactly what the splice path exists to prevent. The owner's
    /// `onCommitEdit` on this path must therefore only clear state, not
    /// call `editTodayBlock`.
    private func commitEditCollab(_ final: String) {
        let trimmed = final.trimmingCharacters(in: .whitespacesAndNewlines)
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

    /// Build the updated property list from the sheet's output and pass
    /// it to `onSetProperties` for the parent to persist.
    private func commitDate(field: DateField, iso: String, time: String?, recurrence: String?) {
        let value = time.map { "\(iso) \($0)" } ?? iso
        let key = field.rawValue  // "deadline" or "scheduled"

        // Upsert: drop any prior value at this key, then append the new one.
        var updated = properties.filter { $0.key != key }
        updated.append(BlockProperty(key: key, value: value))

        if let recurrence {
            updated.removeAll { $0.key == "recurring" }
            updated.append(BlockProperty(key: "recurring", value: recurrence))
        }

        onSetProperties?(updated)
    }
}
