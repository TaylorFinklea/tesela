import SwiftUI
import UIKit

/// Pair-device flow. iOS is currently a thin HTTP client of a server
/// elsewhere, so the QR + 6-character code displayed here are the
/// SERVER's pairing material — fetched from `GET /sync/peer/pairing-code`
/// on whichever backend the user is connected to. The iPhone effectively
/// becomes a mobile vector for sharing the server's pairing code with a
/// third device that happens to be near the phone but not the desktop.
///
/// When iOS is in mock mode (no server attached), the QR card is hidden
/// — the iPhone has no real group state of its own to share yet. The
/// "Scan" and "Type code" paths are still available because they're
/// the joiner direction (no local state needed).
struct PairDeviceView: View {
    @ObservedObject var backend: BackendSettings
    @ObservedObject var mosaic: MockMosaicService

    @Environment(\.theme) private var theme

    @State private var serverCode: MockMosaicService.ServerPairingCode?
    @State private var fetchingCode: Bool = false
    @State private var error: String?
    @State private var showScanner: Bool = false
    @State private var showTypedCode: Bool = false

    private var isAttachedToServer: Bool {
        if case .http = backend.backend { return true }
        return false
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                Text(introCopy)
                    .font(.system(size: 13))
                    .foregroundStyle(theme.fgMuted)

                scanCard

                typedCodeCard

                if isAttachedToServer {
                    qrCard
                } else {
                    notConnectedCard
                }

                Section {
                    pairingSteps
                }
                .padding(.horizontal, 4)
            }
            .padding(.horizontal, 16)
            .padding(.vertical, 20)
        }
        .background(theme.bg)
        .navigationTitle("Pair a device")
        .navigationBarTitleDisplayMode(.inline)
        .task { await refreshCode() }
        .fullScreenCover(isPresented: $showScanner) {
            NavigationStack {
                PairScanView(backend: backend, mosaic: mosaic)
            }
        }
        .sheet(isPresented: $showTypedCode) {
            NavigationStack {
                PairWithShortCodeView(backend: backend, mosaic: mosaic)
            }
            .presentationDetents([.medium])
        }
    }

    // ── Typed-code card (join by typing the 6 digits) ───────────────────

    private var typedCodeCard: some View {
        Button {
            showTypedCode = true
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "keyboard")
                    .font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(theme.accentPrimary)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Type a 6-character code")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(theme.fgDefault)
                    Text("Read the short code from the other device's pairing screen.")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(theme.fgFaint)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(theme.bg2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(theme.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }

    // ── Scan card (join via another device's QR) ────────────────────────

    private var scanCard: some View {
        Button {
            showScanner = true
        } label: {
            HStack(spacing: 12) {
                Image(systemName: "qrcode.viewfinder")
                    .font(.system(size: 22, weight: .semibold))
                    .foregroundStyle(theme.accentPrimary)
                VStack(alignment: .leading, spacing: 2) {
                    Text("Scan a pairing QR")
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(theme.fgDefault)
                    Text("Point at the other device's pairing screen.")
                        .font(.system(size: 11, design: .monospaced))
                        .foregroundStyle(theme.fgFaint)
                }
                Spacer()
                Image(systemName: "chevron.right")
                    .font(.system(size: 12, weight: .semibold))
                    .foregroundStyle(theme.fgFaint)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 12)
            .background(theme.bg2)
            .overlay(
                RoundedRectangle(cornerRadius: 12)
                    .stroke(theme.line, lineWidth: 1)
            )
            .clipShape(RoundedRectangle(cornerRadius: 12))
        }
        .buttonStyle(.plain)
    }

    // ── QR + short code card (server-issued) ────────────────────────────

    private var qrCard: some View {
        VStack(spacing: 14) {
            if let error {
                Text(error)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(theme.typeTask)
                    .multilineTextAlignment(.center)
                    .padding(.horizontal, 12)
            } else if let code = serverCode {
                QRCodeView(payload: code.code)
                    .frame(width: 220, height: 220)
                    .background(theme.fgDefault)
                    .clipShape(RoundedRectangle(cornerRadius: 10))
            } else if fetchingCode {
                ProgressView()
                    .frame(width: 220, height: 220)
            } else {
                Color.clear.frame(height: 220)
            }

            VStack(spacing: 4) {
                Text(formattedShortCode)
                    .font(.system(size: 22, weight: .medium, design: .monospaced))
                    .tracking(4)
                    .foregroundStyle(theme.accentPrimary)
                Text(shortCodeCaption)
                    .font(.system(size: 10.5, design: .monospaced))
                    .foregroundStyle(theme.fgFaint)
            }

            Button {
                Task { await refreshCode() }
            } label: {
                Label("Refresh code", systemImage: "arrow.clockwise")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(theme.fgMuted)
            }
            .disabled(fetchingCode)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 18)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private var notConnectedCard: some View {
        VStack(alignment: .leading, spacing: 6) {
            Text("Not connected to a server")
                .font(.system(size: 14, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text("Pair this iPhone with a server first (use Scan or Type a code above). Once connected, this card will show that mosaic's pairing code so you can bring in a third device.")
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(theme.fgMuted)
        }
        .padding(14)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(theme.bg2)
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.line, lineWidth: 1)
        )
        .clipShape(RoundedRectangle(cornerRadius: 12))
    }

    private var formattedShortCode: String {
        guard let code = serverCode, !code.short_code.isEmpty else { return "—" }
        let s = code.short_code.uppercased()
        guard s.count >= 6 else { return s }
        let mid = s.index(s.startIndex, offsetBy: s.count / 2)
        return "\(s[s.startIndex..<mid]) · \(s[mid..<s.endIndex])"
    }

    private var shortCodeCaption: String {
        guard let code = serverCode else { return "6-character code" }
        let mins = max(1, code.short_code_expires_in_secs / 60)
        return "6-character code · valid \(mins) min"
    }

    // ── How-it-works steps ──────────────────────────────────────────────

    private var pairingSteps: some View {
        VStack(alignment: .leading, spacing: 14) {
            Text("How it works")
                .font(.system(size: 10, design: .monospaced))
                .tracking(1.2)
                .foregroundStyle(theme.fgFaint)

            VStack(alignment: .leading, spacing: 10) {
                step(1, "On the other device, install Tesela and tap Pair.")
                step(2, "Scan this QR or type the 6 digits.")
                step(3, "Approve the request when it appears here.")
                step(4, "Both devices begin syncing automatically.")
            }
        }
    }

    private func step(_ n: Int, _ body: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 12) {
            Text(String(n))
                .font(.system(size: 13, weight: .semibold, design: .monospaced))
                .foregroundStyle(theme.accentPrimary)
                .frame(width: 16, alignment: .leading)
            Text(body)
                .font(.system(size: 13))
                .foregroundStyle(theme.fgMuted)
                .lineSpacing(2)
                .fixedSize(horizontal: false, vertical: true)
            Spacer()
        }
    }

    private var introCopy: String {
        isAttachedToServer
            ? "Show this QR or read the 6 characters to a third device to add it to the same mosaic. Or use Scan / Type code above to point this iPhone at a different server."
            : "Use Scan or Type code below to point this iPhone at a server."
    }

    // ── Pairing code fetch (real handshake material from the server) ────

    /// Fetch the server's pairing code (long base64url + short verifier).
    /// Replaces the earlier iPhone-local code generator, which made fake
    /// material (random group, `tesela://pair` URL) that couldn't pair
    /// anything in practice. The real material lives on the server we're
    /// connected to.
    @MainActor
    private func refreshCode() async {
        guard isAttachedToServer else {
            serverCode = nil
            error = nil
            return
        }
        fetchingCode = true
        defer { fetchingCode = false }
        do {
            serverCode = try await mosaic.fetchPairingCode()
            error = nil
        } catch {
            self.error = "Couldn't reach the server to fetch a pairing code."
            self.serverCode = nil
        }
    }
}

// MARK: - QR rendering

import CoreImage
import CoreImage.CIFilterBuiltins

/// Renders a payload string as a QR code using CoreImage's built-in
/// CIQRCodeGenerator. Black-on-white inside a SwiftUI Image.
struct QRCodeView: View {
    let payload: String

    var body: some View {
        if let img = makeQR() {
            Image(uiImage: img)
                .interpolation(.none)
                .resizable()
                .scaledToFit()
                .padding(12)
        } else {
            // Fallback placeholder if CI fails — should never happen
            // in normal operation.
            Rectangle().fill(Color.gray.opacity(0.3))
        }
    }

    private func makeQR() -> UIImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(payload.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: 8, y: 8))
        let context = CIContext()
        guard let cg = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        return UIImage(cgImage: cg)
    }
}
