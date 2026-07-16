import SwiftUI

private enum ReleaseNotesRoute: Hashable {
    case history
    case detail(String)
}

struct ReleaseNotesView: View {
    let presentation: ReleaseNotesPresentation
    let onCurrentRendered: () -> Void

    @Environment(\.dismiss) private var dismiss
    @Environment(\.theme) private var theme
    @State private var path: [ReleaseNotesRoute] = []

    var body: some View {
        NavigationStack(path: $path) {
            Group {
                if let current = presentation.current {
                    releaseDetail(current, showsHistory: true)
                } else {
                    unavailableView
                }
            }
            .navigationTitle("What’s New")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button("Done") { dismiss() }
                }
            }
            .navigationDestination(for: ReleaseNotesRoute.self) { route in
                switch route {
                case .history:
                    historyView
                case .detail(let id):
                    if let release = presentation.history.first(where: { $0.id == id }) {
                        releaseDetail(release, showsHistory: false)
                    } else {
                        ContentUnavailableView(
                            "Release unavailable",
                            systemImage: "sparkles"
                        )
                    }
                }
            }
        }
        .tint(theme.accentSpark)
        .presentationDragIndicator(.visible)
        .onAppear {
            if presentation.current != nil {
                onCurrentRendered()
            }
        }
        .accessibilityIdentifier("release-notes-sheet")
    }

    private var olderReleases: [ReleaseNote] {
        Array(presentation.history.dropFirst())
    }

    private var unavailableView: some View {
        ContentUnavailableView(
            "Release notes unavailable",
            systemImage: "sparkles",
            description: Text(
                "Tesela is ready to use. Try opening What’s New again after the next update."
            )
        )
        .accessibilityIdentifier("release-notes-unavailable")
    }

    private func releaseDetail(
        _ release: ReleaseNote,
        showsHistory: Bool
    ) -> some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Image(systemName: "sparkles")
                    .font(.system(size: 25, weight: .semibold))
                    .foregroundStyle(theme.accentSpark)
                    .frame(width: 58, height: 58)
                    .background(theme.tint(theme.accentSpark, 12))
                    .overlay(
                        RoundedRectangle(cornerRadius: 18)
                            .stroke(theme.tint(theme.accentSpark, 45), lineWidth: 1)
                    )
                    .clipShape(RoundedRectangle(cornerRadius: 18))
                    .padding(.bottom, 22)

                Text(releaseMeta(release))
                    .font(.system(size: 11, weight: .semibold, design: .monospaced))
                    .tracking(0.6)
                    .textCase(.uppercase)
                    .foregroundStyle(theme.fgFaint)

                Text(release.title)
                    .font(.system(size: 38, weight: .bold, design: .rounded))
                    .foregroundStyle(theme.fgDefault)
                    .padding(.top, 8)

                Text(release.summary)
                    .font(.system(size: 17))
                    .foregroundStyle(theme.fgMuted)
                    .lineSpacing(4)
                    .padding(.top, 10)

                VStack(spacing: 14) {
                    if !release.newItems.isEmpty {
                        ReleaseNotesGroup(
                            title: "New",
                            symbol: "plus",
                            color: theme.typeQuery,
                            items: release.newItems
                        )
                    }
                    if !release.fixed.isEmpty {
                        ReleaseNotesGroup(
                            title: "Fixed",
                            symbol: "checkmark",
                            color: theme.typeProject,
                            items: release.fixed
                        )
                    }
                    if !release.important.isEmpty {
                        ReleaseNotesGroup(
                            title: "Important",
                            symbol: "exclamationmark",
                            color: theme.typeNote,
                            items: release.important
                        )
                    }
                }
                .padding(.top, 34)

                if showsHistory, !olderReleases.isEmpty {
                    Button {
                        path.append(.history)
                    } label: {
                        HStack(spacing: 12) {
                            VStack(alignment: .leading, spacing: 3) {
                                Text("View older releases")
                                    .font(.system(size: 15, weight: .semibold))
                                    .foregroundStyle(theme.fgDefault)
                                Text("\(olderReleases.count) earlier \(olderReleases.count == 1 ? "release" : "releases")")
                                    .font(.system(size: 11, design: .monospaced))
                                    .foregroundStyle(theme.fgFaint)
                            }
                            Spacer()
                            Image(systemName: "chevron.right")
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(theme.fgSubtle)
                        }
                        .padding(17)
                        .background(theme.bg2)
                        .overlay(
                            RoundedRectangle(cornerRadius: 14)
                                .stroke(theme.lineSoft, lineWidth: 1)
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 14))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("release-notes-history-button")
                    .padding(.top, 18)
                }
            }
            .frame(maxWidth: 680, alignment: .leading)
            .padding(.horizontal, 22)
            .padding(.top, 42)
            .padding(.bottom, 56)
            .frame(maxWidth: .infinity)
        }
        .background(
            LinearGradient(
                colors: [theme.tint(theme.accentSpark, 8), theme.bg, theme.bg],
                startPoint: .top,
                endPoint: .center
            )
            .ignoresSafeArea()
        )
        .accessibilityIdentifier(
            release.id == presentation.current?.id
                ? "release-notes-current"
                : "release-notes-detail-\(release.id)"
        )
    }

    private var historyView: some View {
        ScrollView {
            LazyVStack(alignment: .leading, spacing: 12) {
                Text("The changes that led to the version you’re using now.")
                    .font(.system(size: 15))
                    .foregroundStyle(theme.fgMuted)
                    .padding(.bottom, 8)

                ForEach(olderReleases) { release in
                    Button {
                        path.append(.detail(release.id))
                    } label: {
                        HStack(spacing: 14) {
                            VStack(alignment: .leading, spacing: 5) {
                                Text(releaseMeta(release))
                                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                                    .textCase(.uppercase)
                                    .foregroundStyle(theme.fgFaint)
                                Text(release.title)
                                    .font(.system(size: 16, weight: .semibold))
                                    .foregroundStyle(theme.fgDefault)
                                Text(release.summary)
                                    .font(.system(size: 13))
                                    .foregroundStyle(theme.fgMuted)
                                    .lineLimit(2)
                            }
                            Spacer(minLength: 4)
                            Image(systemName: "chevron.right")
                                .font(.caption.weight(.semibold))
                                .foregroundStyle(theme.fgFaint)
                        }
                        .padding(16)
                        .background(theme.bg2)
                        .overlay(
                            RoundedRectangle(cornerRadius: 14)
                                .stroke(theme.lineSoft, lineWidth: 1)
                        )
                        .clipShape(RoundedRectangle(cornerRadius: 14))
                    }
                    .buttonStyle(.plain)
                    .accessibilityIdentifier("release-notes-history-\(release.id)")
                }
            }
            .frame(maxWidth: 680, alignment: .leading)
            .padding(22)
            .frame(maxWidth: .infinity)
        }
        .background(theme.bg.ignoresSafeArea())
        .navigationTitle("Earlier Releases")
        .navigationBarTitleDisplayMode(.inline)
        .accessibilityIdentifier("release-notes-history")
    }

    private func releaseMeta(_ release: ReleaseNote) -> String {
        let date = release.publishedDate?.formatted(
            .dateTime.month(.abbreviated).day().year()
        ) ?? release.publishedAt
        return "\(release.versionLabel(for: presentation.platform)) · \(date)"
    }
}

private struct ReleaseNotesGroup: View {
    let title: String
    let symbol: String
    let color: Color
    let items: [String]

    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            Label(title, systemImage: symbol)
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
                .symbolVariant(.circle.fill)
                .symbolRenderingMode(.palette)
                .foregroundStyle(color, theme.tint(color, 14))

            VStack(alignment: .leading, spacing: 11) {
                ForEach(Array(items.enumerated()), id: \.offset) { _, item in
                    HStack(alignment: .firstTextBaseline, spacing: 10) {
                        Circle()
                            .fill(color)
                            .frame(width: 5, height: 5)
                        Text(item)
                            .font(.system(size: 15))
                            .foregroundStyle(theme.fgMuted)
                            .lineSpacing(3)
                    }
                }
            }
        }
        .padding(19)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 15)
                .stroke(theme.tint(color, 32), lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 15))
    }
}

private struct ReleaseNotesPresentationModifier: ViewModifier {
    @ObservedObject var presenter: ReleaseNotesPresenter
    let onboardingComplete: Bool

    func body(content: Content) -> some View {
        content
            .environment(\.openReleaseNotes, { presenter.presentCurrent() })
            .sheet(item: $presenter.presentation) { presentation in
                ReleaseNotesView(
                    presentation: presentation,
                    onCurrentRendered: presenter.markCurrentRendered
                )
            }
            .onAppear {
                presenter.autoPresentIfNeeded(onboardingComplete: onboardingComplete)
            }
            .onChange(of: onboardingComplete) { _, complete in
                presenter.autoPresentIfNeeded(onboardingComplete: complete)
            }
    }
}

extension View {
    func releaseNotesPresentation(
        presenter: ReleaseNotesPresenter,
        onboardingComplete: Bool
    ) -> some View {
        modifier(
            ReleaseNotesPresentationModifier(
                presenter: presenter,
                onboardingComplete: onboardingComplete
            )
        )
    }
}

private struct OpenReleaseNotesKey: EnvironmentKey {
    static let defaultValue: () -> Void = {}
}

extension EnvironmentValues {
    var openReleaseNotes: () -> Void {
        get { self[OpenReleaseNotesKey.self] }
        set { self[OpenReleaseNotesKey.self] = newValue }
    }
}

#Preview {
    if let catalog = ReleaseNotesCatalogSource.loadBundled(),
       let current = catalog.currentRelease(for: .ios) {
        ReleaseNotesView(
            presentation: ReleaseNotesPresentation(
                catalog: catalog,
                platform: .ios,
                current: current
            ),
            onCurrentRendered: {}
        )
        .environment(\.theme, .graphite)
        .preferredColorScheme(.dark)
    } else {
        ContentUnavailableView("Release notes unavailable", systemImage: "sparkles")
    }
}
