import SwiftUI
import AppKit

extension Color {
    init?(hex: String) {
        var str = hex.trimmingCharacters(in: .whitespacesAndNewlines)
        if str.hasPrefix("#") { str.removeFirst() }
        guard str.count == 6 else { return nil }
        var rgb: UInt64 = 0
        guard Scanner(string: str).scanHexInt64(&rgb) else { return nil }
        self.init(
            red: Double((rgb >> 16) & 0xFF) / 255,
            green: Double((rgb >> 8) & 0xFF) / 255,
            blue: Double(rgb & 0xFF) / 255
        )
    }

    var hexString: String {
        guard let components = NSColor(self).usingColorSpace(.sRGB) else { return "#808080" }
        let r = Int(components.redComponent * 255)
        let g = Int(components.greenComponent * 255)
        let b = Int(components.blueComponent * 255)
        return String(format: "#%02x%02x%02x", r, g, b)
    }
}
