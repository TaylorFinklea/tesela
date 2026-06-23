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
    /// The resolved property/type registry (the `@Published` one off
    /// `MockMosaicService`). Drives select VALUE-chip colors from the
    /// resolved `choiceColors` (Phase 5.6). Defaults to empty so non-block
    /// callers and previews work uncolored.
    var propertyRegistry: PropertyRegistry = PropertyRegistry()
    /// A LIVE source for the property registry, read at call time so the
    /// editor's slash/NLP providers resolve against the current registry even
    /// when a type-page edit lands mid-session (the captured-by-value snapshot
    /// `propertyRegistry` would go stale because the editor's `onAppear` wires
    /// the providers exactly once). Owners wire it to `{ mosaic.propertyRegistry }`.
    /// When nil, the providers fall back to the by-value `propertyRegistry`.
    var registrySource: (() -> PropertyRegistry)? = nil
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
    /// Supplies `[[` page-link suggestions for `query` (the owner wires it
    /// to the service's `searchablePages`). When nil, link autocomplete is
    /// inactive and the toolbar's link button just inserts `[[`. Declared
    /// here (before `onIndent`) to match the call-site argument order.
    var pageSearch: ((String) -> [Page])? = nil
    /// Supplies `#` tag suggestions for `query` (wired to the service's
    /// `searchableTags`). nil → no tag suggestions.
    var tagSearch: ((String) -> [String])? = nil
    /// Apply an indent delta to this block (+1 or -1). Used by the
    /// keyboard accessory toolbar's indent/dedent buttons.
    var onIndent: ((Int) -> Void)? = nil
    /// Cycle the block's kind/status (note → open task → done → note).
    var onCycleStatus: (() -> Void)? = nil
    /// Persist an updated property list for this block. Called after the
    /// date sheet commits — the caller (DailyView/PageView) routes this
    /// to the appropriate service method. Used for the coarse whole-list
    /// path (e.g. a date + recurring set atomically).
    var onSetProperties: (([BlockProperty]) -> Void)? = nil
    /// Typed per-key property write — the STRUCTURED, converging seam
    /// (`setBlockProperty(blockId:key:value:)` → the `BlockPropertySet`
    /// container op). Single-key writes (a lone scheduled/deadline date,
    /// priority, status) route here so they hit the typed converging op
    /// instead of re-pushing the whole property list. Owners wire it to
    /// the service's `setBlockProperty`; when nil, callers fall back to
    /// the whole-list `onSetProperties` path.
    var onSetProperty: ((_ key: String, _ value: String) -> Void)? = nil
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

    // ── Task status + priority ──────────────────────────────────────────
    // The marker itself is the shared `TaskStatusMarker` (status = shape,
    // priority = color, done = green); here we just surface the two raw
    // property strings it needs.

    /// `status::` value (lowercased), or nil when unset.
    private var statusValue: String? {
        let v = properties.first(where: { $0.key.lowercased() == "status" })?
            .value.trimmingCharacters(in: .whitespaces).lowercased()
        return (v?.isEmpty == false) ? v : nil
    }

    /// Raw `priority::` string, or nil — handed to TaskStatusMarker for the color.
    private var priorityValue: String? {
        properties.first(where: { $0.key.lowercased() == "priority" })?.value
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

    /// Resolved property defs for the block's tags, keyed by lowercased
    /// property name — built by unioning `resolvedDefs(forTag:)` across
    /// every tag the block carries (first def wins). Drives select
    /// VALUE-chip colors (Phase 5.6). Empty when no tags / no registry.
    private var resolvedDefsByName: [String: PropertyDef] {
        var map: [String: PropertyDef] = [:]
        for tag in tags {
            let clean = tag.hasPrefix("#") ? String(tag.dropFirst()) : tag
            for def in propertyRegistry.resolvedDefs(forTag: clean) {
                let key = def.name.lowercased()
                if map[key] == nil { map[key] = def }
            }
        }
        return map
    }

    /// The `choiceColors` tint for a displayed select/multi-select property
    /// value — mirrors web `DisplayChip`'s Phase-4 per-choice color. Looks
    /// up the resolved def for `key`, then `choiceColors[value.lowercased()]`
    /// (multi-select colors by the FIRST matching choice). `nil` → the chip
    /// keeps its muted style. The status marker is intentionally NOT routed
    /// here (it stays priority-colored by design).
    private func chipTint(forKey key: String, value: String) -> Color? {
        guard let def = resolvedDefsByName[key.lowercased()] else { return nil }
        guard def.valueType == .select || def.valueType == .multiSelect else { return nil }
        if def.choiceColors.isEmpty { return nil }
        let raw = value.trimmingCharacters(in: .whitespaces)
        guard !raw.isEmpty else { return nil }
        let parts: [String] = def.valueType == .multiSelect
            ? raw.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }
            : [raw]
        for p in parts {
            if let css = def.choiceColors[p.lowercased()],
               let hex = TagPalette.resolveOverride(css) {
                return Color(hex: hex)
            }
        }
        return nil
    }

    @Environment(\.theme) private var theme
    /// Opens the command palette (the `:`/leader stand-in) — the keyboard
    /// toolbar's Commands button calls it. Resolved from the shell.
    @Environment(\.openCommandPalette) private var openCommandPalette
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
    /// Inline autocomplete state ([[ links / # tags / slash verbs) for the
    /// keyboard suggestions strip. The editor coordinator updates it as the
    /// user types; the accessory renders `results` and a pick commits
    /// through `inserter`.
    @StateObject private var editorAutocomplete = EditorAutocomplete()

    @AppStorage("keyboardToolbarItems") private var keyboardToolbarRaw: String = defaultKeyboardToolbarItemsRaw
    @AppStorage("bareDateField") private var bareDateFieldRaw: String = "scheduled"
    @State private var showingDateSheet = false
    /// When a `/scheduled` or `/deadline` slash verb opens the date sheet, this
    /// presets the sheet's field WITHOUT clobbering the user's stored
    /// `bareDateField` default. `nil` falls back to that default.
    @State private var dateSheetFieldPreset: String? = nil

    private var configuredToolbarItems: [KeyboardToolbarItem] {
        decodeKeyboardToolbarItems(keyboardToolbarRaw)
    }

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            bullet
            VStack(alignment: .leading, spacing: 4) {
                content
                if (!tags.isEmpty || recurringValue != nil || deadlineValue != nil || scheduledValue != nil || !displayProperties.isEmpty) && !isEditing {
                    HStack(spacing: 4) {
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
                            PropertyChip(
                                key: prop.key,
                                value: prop.value,
                                def: resolvedDefsByName[prop.key.lowercased()],
                                tint: chipTint(forKey: prop.key, value: prop.value)
                            )
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
                bareDateFieldDefault: dateSheetFieldPreset ?? bareDateFieldRaw,
                onCommit: { field, iso, time, recurrence in
                    commitDate(field: field, iso: iso, time: time, recurrence: recurrence)
                    showingDateSheet = false
                    dateSheetFieldPreset = nil
                },
                onSkip: {
                    onSkipRecurrence?()
                    showingDateSheet = false
                    dateSheetFieldPreset = nil
                },
                onCancel: { showingDateSheet = false; dateSheetFieldPreset = nil }
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
                TaskStatusMarker(
                    status: statusValue,
                    priority: priorityValue,
                    size: 16,
                    onTap: { onToggleTask?() }
                )
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
            autocomplete: editorAutocomplete,
            accessory: collabKeyboardAccessory
        )
        .frame(maxWidth: .infinity, alignment: .leading)
        .onAppear {
            editBuffer = combinedEditableText()
            collabFocused = true
            // Wire the suggestion + NLP sources. They read the registry LIVE
            // (via `registrySource`) on each invocation, so a type-page edit
            // mid-session is reflected without re-running `onAppear`.
            wireAutocompleteSources()
            // Register this editor's imperative inserter so the owner can
            // live-apply an inbound remote splice on THIS block (C1-inbound).
            onActiveCollabInserter?(inserter)
        }
        .onDisappear { editorAutocomplete.dismiss() }
    }

    /// Bind the autocomplete provider + NLP detector stored on the persistent
    /// `editorAutocomplete`. The closures resolve the registry LIVE on each
    /// invocation — via `registrySource()` when wired, else the by-value
    /// `propertyRegistry` snapshot — so a type-page edit that re-publishes the
    /// registry mid-session takes effect for the next keystroke's suggestions
    /// instead of serving the stale snapshot captured when the editor opened.
    private func wireAutocompleteSources() {
        let liveRegistry: () -> PropertyRegistry = { [registrySource, propertyRegistry] in
            registrySource?() ?? propertyRegistry
        }
        editorAutocomplete.provider = { [pageSearch, tagSearch, tags] kind, query in
            Self.suggestions(
                for: kind, query: query,
                pageSearch: pageSearch, tagSearch: tagSearch,
                tags: tags, registry: liveRegistry()
            )
        }
        editorAutocomplete.nlpDetector = { [tags] text, caret in
            InlineNLP.detect(in: text, caretUTF16: caret, tags: tags, registry: liveRegistry())
        }
    }

    /// Build the suggestion chips for a trigger + query. Pages/tags get a
    /// trailing "create new" chip; slash returns the built-in verbs.
    static func suggestions(
        for kind: TriggerKind,
        query: String,
        pageSearch: ((String) -> [Page])?,
        tagSearch: ((String) -> [String])?,
        tags: [String] = [],
        registry: PropertyRegistry = PropertyRegistry()
    ) -> [Suggestion] {
        let q = query.trimmingCharacters(in: .whitespaces)
        switch kind {
        case .link:
            var out = (pageSearch?(query) ?? []).map {
                Suggestion(id: "link:\($0.id)", label: $0.title, insert: "[[\($0.title)]]")
            }
            if !q.isEmpty {
                out.append(Suggestion(id: "link:new", label: "\u{201C}\(q)\u{201D}",
                                      insert: "[[\(q)]]", isCreateNew: true))
            }
            return out
        case .tag:
            var out = (tagSearch?(query) ?? []).map {
                Suggestion(id: "tag:\($0)", label: "#\($0)", insert: "#\($0)")
            }
            if !q.isEmpty, !out.contains(where: { $0.insert == "#\(q)" }) {
                out.append(Suggestion(id: "tag:new", label: "#\(q)", insert: "#\(q)", isCreateNew: true))
            }
            return out
        case .slash:
            // Format verbs + registry-derived property verbs for this block's
            // tags (status/select choices, date openers). P5.4.
            return SlashVerbs.matchingWithRegistry(query, tags: tags, registry: registry)
        case .nlp:
            // The NLP lift suggestion is supplied directly via `updateNLP`;
            // the provider is never consulted for `.nlp`.
            return []
        }
    }

    /// Commit a suggestion — the SINGLE dispatch point (P5.4). Branches on the
    /// suggestion's `action`:
    ///   - `.insertText` keeps the splice (link/tag/format verbs).
    ///   - `.setProperty`/`.setStatus` write the STRUCTURED property via the
    ///     typed per-key seam (NEVER text), then remove the trigger text.
    ///   - `.openDateSheet` removes the trigger text and opens the date sheet
    ///     preset to the field.
    /// After a structured action the trigger span (`startOffset…caret`, the
    /// `/verb` or the matched NLP token) is removed so no raw text remains.
    private func commitSuggestion(_ s: Suggestion) {
        switch s.action {
        case .insertText:
            inserter.replaceTrigger(startOffset: editorAutocomplete.startOffset, with: s.insert)
        case .setProperty(let key, let value):
            inserter.replaceTrigger(startOffset: editorAutocomplete.startOffset, with: "")
            writeProperty(key: key, value: value)
        case .setStatus(let choice):
            inserter.replaceTrigger(startOffset: editorAutocomplete.startOffset, with: "")
            writeProperty(key: "status", value: choice)
        case .openDateSheet(let field):
            inserter.replaceTrigger(startOffset: editorAutocomplete.startOffset, with: "")
            dateSheetFieldPreset = field.rawValue
            showingDateSheet = true
        }
        editorAutocomplete.dismiss()
    }

    /// Write a single structured property through the typed converging seam
    /// (`onSetProperty` → `setBlockProperty` → `BlockPropertySet` container op).
    /// Falls back to the whole-list `onSetProperties` path (upsert by key) when
    /// the typed seam isn't wired, so a property write never silently no-ops.
    private func writeProperty(key: String, value: String) {
        if let onSetProperty {
            onSetProperty(key, value)
            return
        }
        var updated = properties.filter { $0.key != key }
        updated.append(BlockProperty(key: key, value: value))
        onSetProperties?(updated)
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
            // Scrollable middle. While an inline trigger is open ([[ link,
            // # tag, / slash) this slot shows suggestion chips IN PLACE of
            // the format buttons (same pill height — no fragile accessory
            // resizing); otherwise the user-configurable format buttons,
            // scrolling horizontally so the pinned Hide-keyboard button on
            // the right stays reachable.
            ScrollView(.horizontal, showsIndicators: false) {
                HStack(spacing: editorAutocomplete.isActive ? 8 : 18) {
                    if editorAutocomplete.isActive {
                        ForEach(editorAutocomplete.results) { suggestion in
                            suggestionChip(suggestion)
                        }
                    } else {
                        ForEach(scrollableToolbarItems) { item in
                            toolbarButton(for: item)
                        }
                    }
                }
                .padding(.horizontal, 2)
            }
            // Always pinned right — never scrolls, never configurable.
            toolbarButton(for: .hideKeyboard)
        }
    }

    /// A suggestion chip in the inline-trigger strip ([[ page / # tag / slash
    /// verb). "Create new" chips read as distinct (outlined accent); the rest
    /// are filled. Tap → splice the suggestion's insert text.
    private func suggestionChip(_ s: Suggestion) -> some View {
        Button {
            commitSuggestion(s)
        } label: {
            HStack(spacing: 5) {
                Image(systemName: suggestionIcon(s))
                    .font(.system(size: 11, weight: s.isCreateNew ? .semibold : .regular))
                Text(s.label)
                    .font(.system(size: 13, weight: .medium))
                    .lineLimit(1)
            }
            .padding(.horizontal, 11)
            .padding(.vertical, 6)
            .background {
                if s.isCreateNew {
                    RoundedRectangle(cornerRadius: 9)
                        .strokeBorder(theme.accentSecondary.opacity(0.5), lineWidth: 1)
                } else {
                    RoundedRectangle(cornerRadius: 9).fill(theme.bg4)
                }
            }
            .foregroundStyle(s.isCreateNew ? theme.accentSecondary : theme.fgDefault)
        }
        .buttonStyle(.plain)
    }

    /// The SF Symbol for a suggestion chip — "create new" reads as a plus,
    /// structured property/date lifts as a tag/calendar, plain text inserts as
    /// a cursor.
    private func suggestionIcon(_ s: Suggestion) -> String {
        if s.isCreateNew { return "plus" }
        switch s.action {
        case .setProperty, .setStatus: return "tag"
        case .openDateSheet: return "calendar"
        case .insertText: return "text.cursor"
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
                // With autocomplete wired, insert just the `[[` opener so the
                // suggestions strip appears (selecting a page closes the
                // link). Without it, the empty `[[]]` pair as before.
                inserter.insertAtCaret(pageSearch != nil ? "[[" : "[[]]")
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
        case .commandPalette:
            // Open the command palette (the :/leader stand-in). Resolved
            // from the shell environment.
            openCommandPalette()
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

    /// Persist the sheet's output. A LONE date field (no recurrence) is a
    /// single-key change → the typed per-key converging seam
    /// (`onSetProperty`); a date + recurring must land atomically (two
    /// keys) → the whole-list path. Falls back to whole-list when
    /// `onSetProperty` isn't wired.
    private func commitDate(field: DateField, iso: String, time: String?, recurrence: String?) {
        let value = time.map { "\(iso) \($0)" } ?? iso
        let key = field.rawValue  // "deadline" or "scheduled"

        // Single-key fast path: one date, no recurrence → typed seam.
        if recurrence == nil, let onSetProperty {
            onSetProperty(key, value)
            return
        }

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
