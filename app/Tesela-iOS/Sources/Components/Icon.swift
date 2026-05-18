import SwiftUI

/// Tabler-style icon set. Each glyph is hand-drawn as a SwiftUI `Path`
/// on a 24×24 viewBox using 1.5pt strokes with round caps/joins, matching
/// the brand brief's "Tabler iconography only, no SF Symbols" rule.
///
/// Adding a new icon: append a case to `IconName` and add its strokes to
/// the `addStrokes(to:)` switch. Keep paths self-contained — no external
/// SVG parsing — so the file stays greppable and previewable.
enum IconName: String, CaseIterable {
    case plus, mic, search
    case daily, page, tag, hash
    case chevLeft, chevRight, chevDown, chevUp
    case check, pin, more, link
    case cal, bolt, sync, settings
    case trash, archive, share, copyIcon
    case pencil, type, filter
    case sparkles, cloud, wifi, device
    case starIcon, clock
}

/// SwiftUI `Shape` that draws every stroke of an icon on a 24×24 grid.
/// The shape scales to fit whatever rect SwiftUI hands it.
struct IconShape: Shape, @unchecked Sendable {
    let name: IconName

    func path(in rect: CGRect) -> Path {
        let scale = min(rect.width, rect.height) / 24.0
        var p = Path()
        addStrokes(to: &p)
        return p.applying(CGAffineTransform(scaleX: scale, y: scale))
    }

    private func addStrokes(to p: inout Path) {
        switch name {
        case .plus:
            line(&p, (12, 5), (12, 19))
            line(&p, (5, 12), (19, 12))
        case .check:
            polyline(&p, [(5, 12), (10, 17), (20, 7)])
        case .chevLeft:
            polyline(&p, [(15, 6), (9, 12), (15, 18)])
        case .chevRight:
            polyline(&p, [(9, 6), (15, 12), (9, 18)])
        case .chevDown:
            polyline(&p, [(6, 9), (12, 15), (18, 9)])
        case .chevUp:
            polyline(&p, [(18, 15), (12, 9), (6, 15)])
        case .more:
            dot(&p, (5, 12), r: 0.9)
            dot(&p, (12, 12), r: 0.9)
            dot(&p, (19, 12), r: 0.9)
        case .daily, .cal:
            rect(&p, x: 4, y: 5, w: 16, h: 14)
            line(&p, (9, 3), (9, 7))
            line(&p, (15, 3), (15, 7))
            line(&p, (4, 11), (20, 11))
        case .page:
            polyline(&p, [(14, 3), (7, 3), (7, 21), (17, 21), (17, 8), (14, 3)])
            polyline(&p, [(14, 3), (14, 8), (17, 8)])
            line(&p, (9, 13), (15, 13))
            line(&p, (9, 17), (13, 17))
        case .tag:
            polyline(&p, [(3, 12), (3, 5), (12, 3), (21, 12), (12, 21), (3, 12)])
            dot(&p, (8, 8), r: 0.7)
        case .hash:
            line(&p, (4, 9), (20, 9))
            line(&p, (4, 15), (20, 15))
            line(&p, (10, 3), (8, 21))
            line(&p, (16, 3), (14, 21))
        case .search:
            circle(&p, cx: 10, cy: 10, r: 7)
            line(&p, (21, 21), (15, 15))
        case .pin:
            polyline(&p, [(9, 4), (15, 4), (16, 10), (19, 13), (5, 13), (8, 10), (9, 4)])
            line(&p, (12, 13), (12, 21))
        case .link:
            line(&p, (9, 15), (15, 9))
            polyline(&p, [(10, 6), (14, 2), (20, 8), (16, 12)])
            polyline(&p, [(14, 18), (10, 22), (4, 16), (8, 12)])
        case .mic:
            roundedRect(&p, x: 9, y: 3, w: 6, h: 12, r: 3)
            arc(&p, cx: 12, cy: 11, r: 7, start: 0, end: 180)
            line(&p, (12, 18), (12, 21))
        case .bolt:
            polyline(&p, [(13, 3), (4, 14), (11, 14), (10, 21), (19, 10), (12, 10), (13, 3)])
        case .sync:
            arc(&p, cx: 12, cy: 12, r: 9, start: 50, end: 310)
            polyline(&p, [(21, 4), (21, 9), (16, 9)])
        case .settings:
            circle(&p, cx: 12, cy: 12, r: 3)
            circle(&p, cx: 12, cy: 12, r: 8)
        case .trash:
            line(&p, (4, 7), (20, 7))
            line(&p, (10, 11), (10, 17))
            line(&p, (14, 11), (14, 17))
            polyline(&p, [(5, 7), (6, 19), (18, 19), (19, 7)])
            polyline(&p, [(9, 7), (9, 4), (15, 4), (15, 7)])
        case .archive:
            rect(&p, x: 3, y: 6, w: 18, h: 4)
            polyline(&p, [(5, 10), (5, 19), (19, 19), (19, 10)])
            line(&p, (10, 14), (14, 14))
        case .share:
            circle(&p, cx: 6, cy: 12, r: 3)
            circle(&p, cx: 18, cy: 6, r: 3)
            circle(&p, cx: 18, cy: 18, r: 3)
            line(&p, (8.5, 10.5), (15.5, 6.5))
            line(&p, (8.5, 13.5), (15.5, 17.5))
        case .copyIcon:
            rect(&p, x: 9, y: 9, w: 10, h: 10)
            polyline(&p, [(5, 5), (15, 5), (15, 9)])
            polyline(&p, [(5, 5), (5, 15), (9, 15)])
        case .pencil:
            polyline(&p, [(4, 20), (8, 20), (18, 10), (14, 6), (4, 16), (4, 20)])
            line(&p, (13, 6), (17, 10))
        case .type:
            line(&p, (5, 6), (19, 6))
            line(&p, (9, 18), (9, 6))
            line(&p, (5, 18), (13, 18))
        case .filter:
            polyline(&p, [(3, 5), (21, 5), (14, 14), (14, 20), (10, 18), (10, 14), (3, 5)])
        case .sparkles:
            line(&p, (12, 3), (12, 7))
            line(&p, (12, 17), (12, 21))
            line(&p, (3, 12), (7, 12))
            line(&p, (17, 12), (21, 12))
        case .cloud:
            polyline(&p, [(7, 18), (5, 16), (4, 13), (6, 10), (9, 9), (11, 6), (15, 5), (18, 7), (19, 10), (21, 12), (20, 16), (17, 18), (7, 18)])
        case .wifi:
            arc(&p, cx: 12, cy: 9, r: 9, start: -20, end: 200)
            arc(&p, cx: 12, cy: 12.5, r: 5.5, start: -20, end: 200)
            arc(&p, cx: 12, cy: 16, r: 2.5, start: -20, end: 200)
            dot(&p, (12, 19.5), r: 0.6)
        case .device:
            rect(&p, x: 5, y: 3, w: 14, h: 18)
            line(&p, (10, 19), (14, 19))
        case .starIcon:
            polyline(&p, [(12, 3), (15, 9), (22, 10), (17, 15), (18, 22), (12, 19), (6, 22), (7, 15), (2, 10), (9, 9), (12, 3)])
        case .clock:
            circle(&p, cx: 12, cy: 12, r: 9)
            polyline(&p, [(12, 7), (12, 12), (15, 14)])
        }
    }

    // ── Drawing helpers ─────────────────────────────────────────────────

    private func line(_ p: inout Path, _ a: (Double, Double), _ b: (Double, Double)) {
        p.move(to: CGPoint(x: a.0, y: a.1))
        p.addLine(to: CGPoint(x: b.0, y: b.1))
    }

    private func polyline(_ p: inout Path, _ pts: [(Double, Double)]) {
        guard let first = pts.first else { return }
        p.move(to: CGPoint(x: first.0, y: first.1))
        for pt in pts.dropFirst() {
            p.addLine(to: CGPoint(x: pt.0, y: pt.1))
        }
    }

    private func rect(_ p: inout Path, x: Double, y: Double, w: Double, h: Double) {
        p.addRect(CGRect(x: x, y: y, width: w, height: h))
    }

    private func roundedRect(_ p: inout Path, x: Double, y: Double, w: Double, h: Double, r: Double) {
        p.addRoundedRect(in: CGRect(x: x, y: y, width: w, height: h), cornerSize: CGSize(width: r, height: r))
    }

    private func circle(_ p: inout Path, cx: Double, cy: Double, r: Double) {
        p.addEllipse(in: CGRect(x: cx - r, y: cy - r, width: 2 * r, height: 2 * r))
    }

    private func arc(_ p: inout Path, cx: Double, cy: Double, r: Double, start: Double, end: Double) {
        p.addArc(
            center: CGPoint(x: cx, y: cy),
            radius: r,
            startAngle: .degrees(start),
            endAngle: .degrees(end),
            clockwise: false
        )
    }

    private func dot(_ p: inout Path, _ pt: (Double, Double), r: Double = 0.5) {
        p.addEllipse(in: CGRect(x: pt.0 - r, y: pt.1 - r, width: 2 * r, height: 2 * r))
    }
}

struct Icon: View {
    let name: IconName
    var size: CGFloat = 22
    var lineWidth: CGFloat = 1.5

    var body: some View {
        IconShape(name: name)
            .stroke(style: StrokeStyle(lineWidth: lineWidth, lineCap: .round, lineJoin: .round))
            .frame(width: size, height: size)
            .accessibilityHidden(true)
    }
}

/// Tap-target wrapping for an icon button. 44pt min hit area per iOS HIG.
struct IconButton: View {
    let name: IconName
    var size: CGFloat = 22
    var color: Color? = nil
    var action: () -> Void = {}

    @Environment(\.theme) private var theme

    var body: some View {
        Button(action: action) {
            Icon(name: name, size: size)
                .foregroundStyle(color ?? theme.fgMuted)
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }
}
