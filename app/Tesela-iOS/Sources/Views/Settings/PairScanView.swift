import SwiftUI
import AVFoundation
import UIKit

/// Camera-driven QR scanner for the pairing flow. The user points the
/// phone at the desktop's pairing QR; on decode we validate via the FFI
/// `decodePairingCode`, then offer to set the iOS app's backend URL to
/// the inviter's server.
///
/// This is the iOS counterpart to the desktop's "Show pairing code"
/// flow. Today iOS is the thin HTTP client, so "pairing" reduces to
/// "switch backend URL to the scanned host." When iOS gains a local
/// Rust core (per project_mobile_strategy), the same scan callback
/// will fan out into the real cryptographic adoption.
struct PairScanView: View {
    @ObservedObject var backend: BackendSettings
    @ObservedObject var mosaic: MockMosaicService
    @ObservedObject var registry: MosaicRegistry

    /// Success signal threaded down from `PairDeviceView`; see its doc
    /// comment. Fired in `adopt(_:)`, not touched otherwise.
    var onPaired: ((String?) -> Void)? = nil

    @Environment(\.theme) private var theme
    @Environment(\.dismiss) private var dismiss

    /// Scanner lifecycle state.
    @State private var permission: PermissionState = .checking
    /// Pairing code parsed from a successfully decoded QR. Drives the
    /// confirmation sheet.
    @State private var pending: PairingCodeRecord?
    /// Raw scanned code blob (the base64url payload) for the pending record —
    /// needed to cache for the relay tick when pairing a relay-only node.
    @State private var pendingPayload: String?
    @State private var rawError: String?
    /// Set when the capture pipeline fails to build — see
    /// `QRScannerViewController.onSessionError`.
    @State private var cameraUnavailable = false

    enum PermissionState: Equatable {
        case checking
        case granted
        case denied
        case restricted
    }

    var body: some View {
        ZStack {
            theme.bg.ignoresSafeArea()
            content
        }
        .navigationTitle("Scan pairing QR")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
            ToolbarItem(placement: .cancellationAction) {
                Button("Cancel") { dismiss() }
                    .tint(theme.fgMuted)
            }
        }
        .onAppear { evaluatePermission() }
        .sheet(item: $pending) { code in
            confirmSheet(for: code)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch permission {
        case .checking:
            ProgressView()
                .tint(theme.fgMuted)
        case .granted:
            if cameraUnavailable {
                cameraUnavailableCard
            } else {
                scannerSurface
            }
        case .denied:
            permissionDenied
        case .restricted:
            permissionRestricted
        }
    }

    // MARK: Scanner surface

    private var scannerSurface: some View {
        ZStack {
            QRScannerRepresentable(
                onScan: handleScan,
                onSessionError: { cameraUnavailable = true }
            )
            .ignoresSafeArea()

            VStack {
                Spacer()
                Text(rawError ?? "Point at the desktop pairing QR")
                    .font(.system(size: 12, design: .monospaced))
                    .foregroundStyle(rawError == nil ? Color.white.opacity(0.85) : Color.red.opacity(0.9))
                    .padding(.horizontal, 14)
                    .padding(.vertical, 8)
                    .background(Color.black.opacity(0.55), in: Capsule())
                    .padding(.bottom, 36)
            }

            reticle
        }
    }

    private var reticle: some View {
        RoundedRectangle(cornerRadius: 16)
            .stroke(Color.white.opacity(0.7), lineWidth: 2)
            .frame(width: 260, height: 260)
    }

    private var permissionDenied: some View {
        deniedCard(
            title: "Camera access denied",
            body: "Open Settings → Tesela → Camera and allow access, then come back here.",
            primary: "Open Settings",
            primaryAction: openAppSettings
        )
    }

    private var permissionRestricted: some View {
        deniedCard(
            title: "Camera not available",
            body: "This device's camera is restricted (e.g. parental controls). Pairing via QR isn't possible.",
            primary: "Done",
            primaryAction: { dismiss() }
        )
    }

    private var cameraUnavailableCard: some View {
        deniedCard(
            title: "Camera unavailable",
            body: "This iPhone's camera couldn't be started for scanning. Go back and choose \"Type a 6-character code\" to pair instead.",
            primary: "Back",
            primaryAction: { dismiss() }
        )
    }

    private func deniedCard(
        title: String,
        body: String,
        primary: String,
        primaryAction: @escaping () -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title)
                .font(.system(size: 16, weight: .semibold))
                .foregroundStyle(theme.fgDefault)
            Text(body)
                .font(.system(size: 13))
                .foregroundStyle(theme.fgMuted)
            Button(action: primaryAction) {
                Text(primary)
                    .font(.system(size: 14, weight: .semibold))
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 10)
                    .foregroundStyle(theme.bg)
                    .background(theme.accentPrimary)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
            }
            .buttonStyle(.plain)
        }
        .padding(18)
        .background(theme.bg2)
        .clipShape(RoundedRectangle(cornerRadius: 12))
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(theme.line, lineWidth: 1)
        )
        .padding(.horizontal, 24)
    }

    // MARK: Confirmation sheet

    private func confirmSheet(for code: PairingCodeRecord) -> some View {
        NavigationStack {
            Form {
                Section {
                    LabeledContent("Inviter") {
                        Text(code.displayName)
                            .font(.system(.body, design: .monospaced))
                    }
                    LabeledContent("URL") {
                        Text(code.url)
                            .font(.system(size: 12, design: .monospaced))
                            .lineLimit(2)
                    }
                    LabeledContent("Device") {
                        Text(String(code.deviceIdHex.prefix(8)) + "…")
                            .font(.system(.body, design: .monospaced))
                    }
                } header: {
                    Text("Pair with this device")
                } footer: {
                    Text("Saving switches this iPhone's backend to the inviter's server. You can change it back any time in Settings → Backend.")
                        .font(.system(size: 11, design: .monospaced))
                }

                Section {
                    Button {
                        adopt(code)
                    } label: {
                        Text("Pair & connect")
                            .font(.system(size: 14, weight: .semibold))
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.borderedProminent)
                    Button("Cancel") {
                        pending = nil
                    }
                    .tint(.secondary)
                }
            }
            .navigationTitle("Confirm pair")
            .navigationBarTitleDisplayMode(.inline)
        }
        .presentationDetents([.medium])
    }

    // MARK: Permission + scan handling

    private func evaluatePermission() {
        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            permission = .granted
        case .denied:
            permission = .denied
        case .restricted:
            permission = .restricted
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { granted in
                DispatchQueue.main.async {
                    permission = granted ? .granted : .denied
                }
            }
        @unknown default:
            permission = .denied
        }
    }

    private func handleScan(_ payload: String) {
        // Strict-decode: any QR that isn't a valid pairing code is just
        // ignored (camera stays live so the user can keep aiming).
        do {
            let record = try decodePairingCode(code: payload)
            pending = record
            pendingPayload = payload
            rawError = nil
        } catch {
            rawError = "QR not a Tesela pairing code"
        }
    }

    private func adopt(_ code: PairingCodeRecord) {
        // Relay-only inviter (loopback/empty `url` + a relay URL — e.g. the Tauri
        // desktop embed): the phone can't reach the inviter's HTTP server, so
        // pair via the RELAY instead of pointing the thin HTTP client at an
        // unreachable loopback. Cache the code for the relay tick + switch to
        // `.relay` mode so `hubMode` stays off and the relay tick syncs with
        // this code's relay URL; the data layer's local-first read surfaces
        // the synced notes (no Mac reachability needed).
        if RelayTicker.isRelayOnlyPairing(code) {
            if let raw = pendingPayload { RelayTicker.cachePairingCode(raw) }
            // Local-first RELAY mode (not Mock — Mock is the fake snapshot): the
            // UI reads the on-device engine's relay-synced notes while the relay
            // tick syncs in the background. `hubMode` stays off (only `.http`
            // sets it), so the relay tick runs.
            backend.mode = .relay
            pending = nil
            pendingPayload = nil
            onPaired?(code.displayName)
            dismiss()
            return
        }
        // Reachable server → thin HTTP client (current behavior). Switching the
        // backend URL is the meaningful effect of a "pair".
        backend.mode = .http
        backend.serverURL = code.url
        Task {
            // Pairing handoff: pull the inviter server's mosaics into
            // the registry and activate its current one. AppShell sees
            // the activeID change and attaches + loads.
            await registry.importDiscovered(serverURL: code.url, activateCurrent: true)
        }
        pending = nil
        pendingPayload = nil
        onPaired?(code.displayName)
        dismiss()
    }

    private func openAppSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
    }
}

/// Make `PairingCodeRecord` selectable for `.sheet(item:)`. The FFI
/// type isn't Identifiable on its own.
extension PairingCodeRecord: Identifiable {
    public var id: String { deviceIdHex + url }
}

// MARK: - UIViewControllerRepresentable wrapper

/// Wraps an `AVCaptureSession` + preview layer. Delivers each decoded
/// QR string to `onScan`. The session pauses after a single successful
/// decode to avoid spamming the same payload while the user is staring
/// at it — the confirmation sheet drives the next step.
struct QRScannerRepresentable: UIViewControllerRepresentable {
    let onScan: (String) -> Void
    let onSessionError: () -> Void

    func makeUIViewController(context: Context) -> QRScannerViewController {
        let vc = QRScannerViewController()
        vc.onScan = onScan
        vc.onSessionError = onSessionError
        return vc
    }

    func updateUIViewController(_: QRScannerViewController, context _: Context) {}
}

final class QRScannerViewController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var onScan: ((String) -> Void)?
    /// Called on the main queue when the capture pipeline can't be built
    /// (no camera, input/output rejected). Session setup runs off the main
    /// thread now, so this failure is asynchronous — without it the user
    /// would just stare at a permanent black screen.
    var onSessionError: (() -> Void)?

    private let session = AVCaptureSession()
    /// Every `AVCaptureSession` call — configuration and start/stop — runs
    /// on this one serial queue. Adding a camera input and `startRunning()`
    /// both block; keeping them off the main thread is what stops the
    /// pairing screen from freezing. (Apple's AVCam sample pattern.)
    private let sessionQueue = DispatchQueue(label: "app.tesela.qr-scanner.session")
    private var preview: AVCaptureVideoPreviewLayer?
    private var lastPayload: String?
    private var lastPayloadAt: Date = .distantPast

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .black

        // The preview layer is a UI object: create and attach it on the
        // main thread. It shows nothing until the session starts running.
        let preview = AVCaptureVideoPreviewLayer(session: session)
        preview.videoGravity = .resizeAspectFill
        preview.frame = view.bounds
        view.layer.addSublayer(preview)
        self.preview = preview

        sessionQueue.async { [weak self] in self?.configureSession() }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        preview?.frame = view.bounds
    }

    override func viewWillAppear(_ animated: Bool) {
        super.viewWillAppear(animated)
        sessionQueue.async { [weak self] in
            guard let self, !self.session.isRunning else { return }
            self.session.startRunning()
        }
    }

    override func viewWillDisappear(_ animated: Bool) {
        super.viewWillDisappear(animated)
        sessionQueue.async { [weak self] in
            guard let self, self.session.isRunning else { return }
            self.session.stopRunning()
        }
    }

    /// Runs on `sessionQueue`, never the main thread.
    private func configureSession() {
        guard
            let device = AVCaptureDevice.default(for: .video),
            let input = try? AVCaptureDeviceInput(device: device),
            session.canAddInput(input)
        else {
            reportSessionError()
            return
        }

        session.beginConfiguration()
        session.addInput(input)

        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else {
            session.commitConfiguration()
            reportSessionError()
            return
        }
        session.addOutput(output)
        output.metadataObjectTypes = [.qr]
        output.setMetadataObjectsDelegate(self, queue: .main)
        session.commitConfiguration()
    }

    private func reportSessionError() {
        DispatchQueue.main.async { [weak self] in self?.onSessionError?() }
    }

    func metadataOutput(
        _: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from _: AVCaptureConnection
    ) {
        guard
            let object = metadataObjects.first as? AVMetadataMachineReadableCodeObject,
            object.type == .qr,
            let payload = object.stringValue,
            !payload.isEmpty
        else { return }
        // Debounce identical scans within 1.5s — the SwiftUI confirmation
        // sheet is what gates the next step; we don't want repeat fires
        // while the user is reading.
        let now = Date()
        if payload == lastPayload, now.timeIntervalSince(lastPayloadAt) < 1.5 {
            return
        }
        lastPayload = payload
        lastPayloadAt = now
        onScan?(payload)
    }
}
