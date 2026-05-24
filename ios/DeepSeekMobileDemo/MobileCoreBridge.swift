import Combine
import Foundation

@MainActor
protocol MobileCoreBridge: ObservableObject {
    var sessions: [MobileSession] { get }
    var selectedSessionID: UUID? { get set }
    var connectionSettings: ConnectionSettings { get set }
    var auditEvents: [AuditEvent] { get }

    func messages(for sessionID: UUID) -> [ChatMessage]
    func pendingCommand(for sessionID: UUID) -> PendingCommand?
    func browserSession(for sessionID: UUID) -> MockBrowserSession?
    func maintenanceApproval(for sessionID: UUID) -> MaintenanceApprovalRequest?
    func createSession(title: String)
    func sendMessage(_ text: String, in sessionID: UUID)
    func decide(commandID: UUID, decision: CommandDecision)
    func decide(browserClickApprovalID: UUID, decision: CommandDecision)
    func decide(maintenanceApprovalID: UUID, decision: CommandDecision)
    func saveConnectionSettings()
    func importBootstrapOutput(_ text: String)
    func discoverRunnerCapabilities()
    func runnerRequestAuthMetadata() -> RunnerRequestAuthMetadata
    @discardableResult
    func requestPairingCode() -> String
    @discardableResult
    func redeemPairingCode(_ code: String) -> Bool
    @discardableResult
    func upgradePairing(endpoint: String, pairingToken: String, capabilitiesJSON: String) -> RunnerProfile
}

@MainActor
final class MockMobileCoreBridge: MobileCoreBridge {
    @Published var sessions: [MobileSession]
    @Published var selectedSessionID: UUID?
    @Published var connectionSettings: ConnectionSettings
    @Published var auditEvents: [AuditEvent]

    private var messagesBySession: [UUID: [ChatMessage]]
    private var pendingCommandsBySession: [UUID: PendingCommand]
    private var browserSessionsBySession: [UUID: MockBrowserSession]
    private var maintenanceRequestsBySession: [UUID: MaintenanceApprovalRequest]
    private var nextRunnerApprovalNonce: MockIssuedApprovalNonce?
    private var nextBrowserClickApprovalNonce: MockIssuedApprovalNonce?
    private let coreBridgeAdapter: CoreBridgeJSONAdapter?
    private let credentialStore: CredentialStore?
    private let storeAdapter: MobileStoreAdapter?

    init(
        snapshot: DemoSnapshot = MockMobileCoreBridge.makeDemoSnapshot(),
        coreUniFFIProvider: CoreBridgeUniFFIJSONProvider? = nil,
        coreJSONAPI: CoreBridgeJSONAPI? = nil,
        credentialStore: CredentialStore? = nil,
        storeAdapter: MobileStoreAdapter? = nil
    ) {
        let provider = coreUniFFIProvider ?? coreJSONAPI
        let adapter = provider.map { CoreBridgeJSONAdapter(provider: $0) }
        let initialState = Self.makeInitialState(
            snapshot: snapshot,
            coreBridgeAdapter: adapter
        )
        sessions = initialState.snapshot.sessions
        selectedSessionID = initialState.snapshot.sessions.first?.id
        connectionSettings = initialState.connectionSettings
        auditEvents = initialState.snapshot.auditEvents
        messagesBySession = initialState.snapshot.messagesBySession
        pendingCommandsBySession = initialState.snapshot.pendingCommandsBySession
        browserSessionsBySession = initialState.snapshot.browserSessionsBySession
        maintenanceRequestsBySession = initialState.snapshot.maintenanceRequestsBySession
        coreBridgeAdapter = adapter
        self.credentialStore = credentialStore
        self.storeAdapter = storeAdapter
    }

    private static func makeInitialState(
        snapshot: DemoSnapshot,
        coreBridgeAdapter: CoreBridgeJSONAdapter?
    ) -> CoreBridgeInitialState {
        guard let coreBridgeAdapter else {
            return CoreBridgeInitialState(snapshot: snapshot, connectionSettings: .demo)
        }

        do {
            return try coreBridgeAdapter.initialState(
                fallbackSnapshot: snapshot,
                fallbackSettings: .demo
            )
        } catch {
            return CoreBridgeInitialState(snapshot: snapshot, connectionSettings: .demo)
        }
    }

    func messages(for sessionID: UUID) -> [ChatMessage] {
        messagesBySession[sessionID, default: []]
    }

    func pendingCommand(for sessionID: UUID) -> PendingCommand? {
        pendingCommandsBySession[sessionID]
    }

    func browserSession(for sessionID: UUID) -> MockBrowserSession? {
        browserSessionsBySession[sessionID]
    }

    func maintenanceApproval(for sessionID: UUID) -> MaintenanceApprovalRequest? {
        maintenanceRequestsBySession[sessionID]
    }

    func createSession(title: String) {
        let session = MobileSession(
            id: UUID(),
            title: title,
            subtitle: "New mobile shell session",
            status: .ready,
            updatedAt: Date(),
            unreadCount: 0
        )
        sessions.insert(session, at: 0)
        selectedSessionID = session.id
        messagesBySession[session.id] = [
            ChatMessage(
                id: UUID(),
                sessionID: session.id,
                role: .system,
                body: "Session created. Mobile core bridge is using demo data.",
                timestamp: Date(),
                isThinking: false
            )
        ]
        appendAudit(.session, title: "Created session", detail: title)
    }

    func sendMessage(_ text: String, in sessionID: UUID) {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            return
        }

        messagesBySession[sessionID, default: []].append(
            ChatMessage(
                id: UUID(),
                sessionID: sessionID,
                role: .user,
                body: text,
                timestamp: Date(),
                isThinking: false
            )
        )
        messagesBySession[sessionID, default: []].append(
            ChatMessage(
                id: UUID(),
                sessionID: sessionID,
                role: .assistant,
                body: "Mock response queued. Replace MockMobileCoreBridge with UniFFI-backed mobile core when bindings are generated.",
                timestamp: Date(),
                isThinking: false
            )
        )
        touchSession(sessionID, status: .ready)
    }

    func decide(commandID: UUID, decision: CommandDecision) {
        guard let pair = pendingCommandsBySession.first(where: { $0.value.id == commandID }) else {
            return
        }

        pendingCommandsBySession[pair.key] = nil
        let title: String
        let detail: String
        switch decision {
        case .approved:
            let nonce = issueApprovalNonce(for: pair.value)
            title = "Approved command"
            detail = "\(pair.value.risk.rawValue) risk - \(pair.value.command) - cwd \(pair.value.workingDirectory) - approval nonce issued and \(nonce.status.rawValue.lowercased()) as \(nonce.label)"
        case .denied:
            title = "Denied command"
            detail = "\(pair.value.risk.rawValue) risk - \(pair.value.command) - cwd \(pair.value.workingDirectory) - approval nonce not issued"
        }

        appendAudit(
            .command,
            title: title,
            detail: detail
        )
        touchSession(pair.key, status: .ready)
    }

    func decide(browserClickApprovalID: UUID, decision: CommandDecision) {
        guard let pair = browserSessionsBySession.first(where: { sessionPair in
            sessionPair.value.pendingClickApproval?.id == browserClickApprovalID
        }),
        var browserSession = browserSessionsBySession[pair.key],
        let approval = browserSession.pendingClickApproval else {
            return
        }

        let title: String
        let detail: String
        switch decision {
        case .approved:
            let nonce = issueBrowserClickApprovalNonce(for: approval)
            browserSession.status = .clickApproved
            browserSession.clickApprovalNonceLabel = nonce.label
            browserSession.clickApprovalNonceStatus = nonce.status
            title = "Issued browser click approval"
            detail = "\(browserSession.id) - \(approval.targetDescription) - approval nonce issued and \(nonce.status.rawValue.lowercased())"
        case .denied:
            browserSession.status = .clickDenied
            browserSession.clickApprovalNonceLabel = "Not issued"
            browserSession.clickApprovalNonceStatus = .notIssued
            title = "Denied browser click approval"
            detail = "\(browserSession.id) - \(approval.targetDescription) - approval nonce not issued"
        }

        browserSession.pendingClickApproval = nil
        browserSessionsBySession[pair.key] = browserSession
        appendAudit(.browser, title: title, detail: detail)
        touchSession(pair.key, status: .ready)
    }

    func decide(maintenanceApprovalID: UUID, decision: CommandDecision) {
        guard let pair = maintenanceRequestsBySession.first(where: { $0.value.id == maintenanceApprovalID }) else {
            return
        }

        var request = pair.value
        let title: String
        switch decision {
        case .approved:
            request.approvalStatus = .approved
            request.auditSummary = "Dry-run sent for \(request.connectionIDLabel); nonce redacted; audit updated. No update or uninstall executed."
            title = "Approved maintenance dry-run"
        case .denied:
            request.approvalStatus = .denied
            request.auditSummary = "Dry-run denied for \(request.connectionIDLabel); no runner maintenance request was sent."
            title = "Denied maintenance dry-run"
        }

        maintenanceRequestsBySession[pair.key] = request
        appendAudit(
            .maintenance,
            title: title,
            detail: "\(request.operation.rawValue) - \(request.risk.rawValue) risk - \(request.auditSummary)"
        )
        touchSession(pair.key, status: .ready)
    }

    func saveConnectionSettings() {
        persistDemoCredentialIfAvailable()
        appendAudit(
            .connection,
            title: "Saved connection",
            detail: "\(connectionSettings.mode.rawValue) - \(connectionSettings.statusSummary)"
        )
    }

    func importBootstrapOutput(_ text: String) {
        let redactedText = Self.redactedBootstrapDisplayText(text)
        let report = Self.parseRunnerCapabilityReport(from: redactedText)

        connectionSettings.bootstrapOutput = redactedText
        if let endpoint = report.endpoint {
            connectionSettings.endpoint = endpoint
        }

        if report.isOfflineFallback {
            connectionSettings.mode = .bootstrap
            connectionSettings.pairedAccountLabel = ""
            connectionSettings.discoveredRunnerCapabilities = []
            appendAudit(
                .bootstrap,
                title: "Imported offline bootstrap fallback",
                detail: "Bootstrap handoff imported; offline fallback retained bootstrap mode; secrets redacted before display"
            )
            return
        }

        if report.canUpgradeToRunner {
            connectionSettings.mode = .runner
            connectionSettings.pairedAccountLabel = report.accountLabel ?? "Imported runner handoff"
            connectionSettings.tokenLabel = connectionSettings.pairedAccountLabel
            connectionSettings.pendingPairingCode = ""
            connectionSettings.discoveredRunnerCapabilities = report.capabilities
            appendAudit(
                .bootstrap,
                title: "Imported runner capability report",
                detail: "Bootstrap handoff promoted to runner mode; \(report.capabilities.count) capabilities enabled; secrets redacted before display"
            )
            openMockBrowserSessionIfReady()
            return
        }

        appendAudit(
            .bootstrap,
            title: "Imported bootstrap output",
            detail: "Bootstrap handoff imported; \(redactedText.count) characters; secrets redacted before display"
        )
    }

    func discoverRunnerCapabilities() {
        connectionSettings.discoveredRunnerCapabilities = Self.documentedRunnerCapabilities
        appendAudit(
            .connection,
            title: "Discovered capabilities",
            detail: "\(connectionSettings.mode.rawValue) /capabilities - \(Self.documentedRunnerCapabilities.map(\.name).joined(separator: ", "))"
        )
        openMockBrowserSessionIfReady()
    }

    func runnerRequestAuthMetadata() -> RunnerRequestAuthMetadata {
        guard connectionSettings.mode == .runner || connectionSettings.mode == .remoteMcp else {
            return .unavailable(for: connectionSettings)
        }

        let account = connectionSettings.pairedAccountLabel.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !account.isEmpty else {
            return .unpaired(endpoint: connectionSettings.endpoint)
        }

        return .ready(
            endpoint: connectionSettings.endpoint,
            tokenAccountLabel: account,
            approvalNonce: nextRunnerApprovalNonce?.summary
        )
    }

    private func issueApprovalNonce(for command: PendingCommand) -> ApprovalNonceSummary {
        let summary = ApprovalNonceSummary(
            label: Self.makeApprovalNonceLabel(for: command),
            status: .bound
        )
        nextRunnerApprovalNonce = MockIssuedApprovalNonce(
            rawValue: Self.makeApprovalNonceSecret(),
            summary: summary
        )
        return summary
    }

    private func issueBrowserClickApprovalNonce(for approval: BrowserClickApproval) -> ApprovalNonceSummary {
        let summary = ApprovalNonceSummary(
            label: Self.makeBrowserClickApprovalNonceLabel(for: approval),
            status: .bound
        )
        nextBrowserClickApprovalNonce = MockIssuedApprovalNonce(
            rawValue: Self.makeBrowserClickApprovalNonceSecret(),
            summary: summary
        )
        return summary
    }

    @discardableResult
    func requestPairingCode() -> String {
        let code = Self.makePairingCode()
        connectionSettings.pendingPairingCode = code
        connectionSettings.pairedAccountLabel = ""
        connectionSettings.discoveredRunnerCapabilities = []
        nextRunnerApprovalNonce = nil
        nextBrowserClickApprovalNonce = nil
        if let sessionID = selectedSessionID {
            browserSessionsBySession[sessionID] = nil
        }
        appendAudit(
            .connection,
            title: "Requested pairing code",
            detail: "\(connectionSettings.mode.rawValue) mock pairing code generated"
        )
        return code
    }

    @discardableResult
    func redeemPairingCode(_ code: String) -> Bool {
        let submittedCode = code.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !submittedCode.isEmpty,
              submittedCode == connectionSettings.pendingPairingCode else {
            appendAudit(
                .connection,
                title: "Pairing code rejected",
                detail: "\(connectionSettings.mode.rawValue) mock pairing attempt did not match the active code"
            )
            return false
        }

        let account = Self.demoPairingAccountLabel(for: connectionSettings.mode)
        connectionSettings.pendingPairingCode = ""
        connectionSettings.pairedAccountLabel = account
        connectionSettings.tokenLabel = account
        persistDemoCredential(account: account, secret: Self.makeDemoPairingToken())
        appendAudit(
            .connection,
            title: "Pairing code redeemed",
            detail: "\(connectionSettings.mode.rawValue) paired with credential label \(account)"
        )
        discoverRunnerCapabilities()
        return true
    }

    @discardableResult
    func upgradePairing(endpoint: String, pairingToken: String, capabilitiesJSON: String) -> RunnerProfile {
        let normalizedEndpoint = endpoint.trimmingCharacters(in: .whitespacesAndNewlines)
        let normalizedToken = pairingToken.trimmingCharacters(in: .whitespacesAndNewlines)
        let submittedCapabilitiesJSON = capabilitiesJSON.trimmingCharacters(in: .whitespacesAndNewlines)
        let fallbackAdapter = CoreBridgeJSONAdapter(provider: FailingPairingUpgradeProvider())
        let profile = (coreBridgeAdapter ?? fallbackAdapter).pairingUpgradeProfile(
            endpoint: normalizedEndpoint.isEmpty ? connectionSettings.endpoint : normalizedEndpoint,
            pairingToken: normalizedToken,
            capabilitiesJSON: submittedCapabilitiesJSON
        )

        connectionSettings.mode = .runner
        connectionSettings.endpoint = profile.endpoint
        connectionSettings.pendingPairingCode = ""
        connectionSettings.pairedAccountLabel = profile.accountLabel
        connectionSettings.tokenLabel = profile.tokenLabel
        connectionSettings.discoveredRunnerCapabilities = profile.capabilities
        persistDemoCredential(account: profile.tokenLabel, secret: Self.demoCredentialPlaceholder)

        let fallbackLabel = profile.isMockFallback ? " with mock fallback" : ""
        appendAudit(
            .connection,
            title: "Upgraded pairing bridge",
            detail: "CoreBridge pairing upgrade\(fallbackLabel); \(profile.capabilities.count) capabilities enabled; pairing token redacted before display"
        )
        openMockBrowserSessionIfReady()
        return profile
    }

    private func openMockBrowserSessionIfReady() {
        guard connectionSettings.isPaired,
              connectionSettings.discoveredRunnerCapabilities.contains(where: { $0.name == "browser" }),
              let sessionID = selectedSessionID,
              browserSessionsBySession[sessionID] == nil else {
            return
        }

        let browserID = Self.makeBrowserSessionID()
        let approval = BrowserClickApproval(
            id: UUID(),
            browserSessionID: browserID,
            targetDescription: "Continue in paired runner",
            pageURL: "https://runner.example.test/mobile/continue",
            rationale: "Mock browser automation wants to click a post-pairing continuation control.",
            approvalNonceLabel: "Issued only after browser click approval",
            approvalNonceStatus: .willIssueOnApproval
        )
        let browserSession = MockBrowserSession(
            id: browserID,
            sessionID: sessionID,
            status: .waitingForClickApproval,
            pageTitle: "Runner pairing complete",
            pageURL: approval.pageURL,
            extractedTextPreview: "Runner pairing ready. Browser automation found a Continue button for the paired mobile session.",
            clickApprovalNonceLabel: approval.approvalNonceLabel,
            clickApprovalNonceStatus: approval.approvalNonceStatus,
            pendingClickApproval: approval
        )

        browserSessionsBySession[sessionID] = browserSession
        appendAudit(
            .browser,
            title: "Opened browser session",
            detail: "\(browserID) - \(browserSession.pageTitle) - mock browser automation only"
        )
        appendAudit(
            .browser,
            title: "Extracted browser text",
            detail: "\(browserID) - preview \(browserSession.extractedTextPreview?.count ?? 0) characters"
        )
        appendAudit(
            .browser,
            title: "Requested browser click approval",
            detail: "\(browserID) - \(approval.targetDescription) - approval nonce pending"
        )
        touchSession(sessionID, status: .waitingForApproval)
    }

    private func touchSession(_ sessionID: UUID, status: SessionStatus) {
        guard let index = sessions.firstIndex(where: { $0.id == sessionID }) else {
            return
        }
        sessions[index].status = status
        sessions[index].updatedAt = Date()
        let updatedSession = sessions.remove(at: index)
        sessions.insert(updatedSession, at: 0)
    }

    private func appendAudit(_ kind: AuditEventKind, title: String, detail: String) {
        let event = AuditEvent(id: UUID(), timestamp: Date(), kind: kind, title: title, detail: detail)
        auditEvents.insert(event, at: 0)
        persistAuditEvent(event)
    }

    private func persistAuditEvent(_ event: AuditEvent) {
        guard let storeAdapter else {
            return
        }

        do {
            try storeAdapter.recordAuditEvent(event)
        } catch {
            auditEvents.insert(
                AuditEvent(
                    id: UUID(),
                    timestamp: Date(),
                    kind: .connection,
                    title: "Audit persistence warning",
                    detail: "Could not persist audit event \(event.id.uuidString): \(error)"
                ),
                at: 0
            )
        }
    }

    private func persistDemoCredentialIfAvailable() {
        let account = connectionSettings.tokenLabel.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !account.isEmpty else {
            return
        }

        persistDemoCredential(account: account, secret: Self.demoCredentialPlaceholder)
    }

    private func persistDemoCredential(account: String, secret: String) {
        let account = account.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !account.isEmpty else {
            return
        }

        guard let credentialStore else {
            appendAudit(
                .credential,
                title: "Credential label retained",
                detail: "No credential store configured; secret value was not retained in memory"
            )
            return
        }

        do {
            try credentialStore.saveSecret(secret, account: account)
            appendAudit(
                .credential,
                title: "Credential label stored",
                detail: "Stored demo credential in configured credential store"
            )
        } catch {
            appendAudit(
                .credential,
                title: "Credential storage warning",
                detail: "Could not store demo credential: \(error)"
            )
        }
    }

    private static func makeDemoSnapshot() -> DemoSnapshot {
        let planningID = UUID()
        let approvalID = UUID()
        let maintenanceID = UUID()
        let uninstallMaintenanceID = UUID()

        let sessions = [
            MobileSession(
                id: planningID,
                title: "Mobile porting plan",
                subtitle: "SwiftUI shell and bridge handoff",
                status: .ready,
                updatedAt: Date(),
                unreadCount: 0
            ),
            MobileSession(
                id: approvalID,
                title: "Local command review",
                subtitle: "One command waiting for approval",
                status: .waitingForApproval,
                updatedAt: Date().addingTimeInterval(-900),
                unreadCount: 1
            ),
            MobileSession(
                id: maintenanceID,
                title: "Runner maintenance",
                subtitle: "Self-update dry-run waiting for approval",
                status: .waitingForApproval,
                updatedAt: Date().addingTimeInterval(-1_200),
                unreadCount: 1
            ),
            MobileSession(
                id: uninstallMaintenanceID,
                title: "Runner uninstall plan",
                subtitle: "Uninstall maintenance dry-run planned",
                status: .ready,
                updatedAt: Date().addingTimeInterval(-1_500),
                unreadCount: 0
            )
        ]

        let messages = [
            planningID: [
                ChatMessage(
                    id: UUID(),
                    sessionID: planningID,
                    role: .assistant,
                    body: "I can keep the mobile app shell usable while UniFFI bindings are still pending.",
                    timestamp: Date().addingTimeInterval(-360),
                    isThinking: false
                ),
                ChatMessage(
                    id: UUID(),
                    sessionID: planningID,
                    role: .user,
                    body: "Show sessions, chat, approvals, setup, bootstrap input, and audit history.",
                    timestamp: Date().addingTimeInterval(-180),
                    isThinking: false
                )
            ],
            approvalID: [
                ChatMessage(
                    id: UUID(),
                    sessionID: approvalID,
                    role: .assistant,
                    body: "A command needs review before it can run on the device.",
                    timestamp: Date().addingTimeInterval(-960),
                    isThinking: false
                )
            ],
            maintenanceID: [
                ChatMessage(
                    id: UUID(),
                    sessionID: maintenanceID,
                    role: .assistant,
                    body: "Runner maintenance has a high-risk dry-run plan that needs mobile approval before the runner can continue.",
                    timestamp: Date().addingTimeInterval(-1_260),
                    isThinking: false
                )
            ],
            uninstallMaintenanceID: [
                ChatMessage(
                    id: UUID(),
                    sessionID: uninstallMaintenanceID,
                    role: .assistant,
                    body: "Runner uninstall is planned as a high-risk dry-run. The UI shows the audit trail without exposing credentials or approval nonces.",
                    timestamp: Date().addingTimeInterval(-1_560),
                    isThinking: false
                )
            ]
        ]

        let pendingCommand = PendingCommand(
            id: UUID(),
            sessionID: approvalID,
            title: "Inspect repository status",
            command: "git status --short",
            workingDirectory: "/workspace/DeepSeek-TUI",
            risk: .low,
            rationale: "Read-only repository state check for mobile shell integration.",
            lease: CommandLeaseApproval(
                leaseIDLabel: "lease-7F3A-CLI",
                idempotencyKeyLabel: "idem-status-7F3A",
                expiresAt: Date().addingTimeInterval(5 * 60),
                approvedActionSummary: "Run read-only shell command: git status --short"
            ),
            approvalNonceLabel: "Issued only after approval",
            approvalNonceStatus: .willIssueOnApproval
        )

        let maintenanceRequest = MaintenanceApprovalRequest(
            id: UUID(),
            sessionID: maintenanceID,
            operation: .selfUpdate,
            title: "Approve runner self-update dry-run",
            connectionIDLabel: "runner-7F3A...C91B",
            targetSummary: "deepseek-tui runner 0.6.6 -> 0.6.7",
            risk: .high,
            approvalStatus: .waitingForDryRunApproval,
            dryRunSteps: [
                MaintenanceDryRunStep(
                    id: UUID(),
                    title: "Verify signed release manifest",
                    detail: "Check version, platform, and signature metadata before any file changes.",
                    destructive: false
                ),
                MaintenanceDryRunStep(
                    id: UUID(),
                    title: "Stage replacement binary",
                    detail: "Download to a temporary runner-managed path and compare checksum.",
                    destructive: false
                ),
                MaintenanceDryRunStep(
                    id: UUID(),
                    title: "Plan service restart",
                    detail: "Would stop the active runner and swap the binary after a separate execution approval.",
                    destructive: true
                )
            ],
            artifactVerification: ArtifactVerificationPlan(
                artifactName: "deepseek-tui-aarch64-apple-darwin.tar.gz",
                checksumSource: "Release manifest sha256",
                signatureSource: "Minisign release signature",
                verifyStep: "Verify checksum and signature metadata before staging the replacement.",
                rollbackGuidance: "Keep the current runner binary active until a later execution approval succeeds."
            ),
            auditSummary: "Planned high-risk maintenance dry-run; no update has been executed.",
            requestedBy: "paired mobile session"
        )

        let uninstallMaintenanceRequest = MaintenanceApprovalRequest(
            id: UUID(),
            sessionID: uninstallMaintenanceID,
            operation: .uninstall,
            title: "Review runner uninstall dry-run",
            connectionIDLabel: "runner-29B1...A044",
            targetSummary: "Remove paired runner service and local support files",
            risk: .high,
            approvalStatus: .planned,
            dryRunSteps: [
                MaintenanceDryRunStep(
                    id: UUID(),
                    title: "Inspect installed service",
                    detail: "Resolve service name, binary path, and launch metadata.",
                    destructive: false
                ),
                MaintenanceDryRunStep(
                    id: UUID(),
                    title: "List removable files",
                    detail: "Report runner-owned files that would be removed after explicit approval.",
                    destructive: false
                ),
                MaintenanceDryRunStep(
                    id: UUID(),
                    title: "Plan credential cleanup",
                    detail: "Would revoke the paired runner connection without displaying token material.",
                    destructive: true
                )
            ],
            artifactVerification: ArtifactVerificationPlan(
                artifactName: "Installed runner service and support files",
                checksumSource: "Local install manifest",
                signatureSource: "Original install metadata",
                verifyStep: "Verify installed paths and ownership before listing removable files.",
                rollbackGuidance: "Leave service files untouched unless a later execution approval confirms removal."
            ),
            auditSummary: "Uninstall dry-run planned; execution requires a later high-risk approval.",
            requestedBy: "device owner"
        )

        let auditEvents = [
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-120),
                kind: .session,
                title: "Selected session",
                detail: "Mobile porting plan"
            ),
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-240),
                kind: .browser,
                title: "Browser rescue captured",
                detail: "Browser runner session recorded recovery text; click approval pending; nonce not shown"
            ),
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-360),
                kind: .windows,
                title: "Windows rescue fallback",
                detail: "PowerShell bootstrap fallback planned; medium risk; user should retry after network access returns"
            ),
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-420),
                kind: .bootstrap,
                title: "Bootstrap handoff reviewed",
                detail: "macOS runner bootstrap handoff imported; endpoint and pairing readiness recorded; raw secret omitted"
            ),
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-600),
                kind: .connection,
                title: "Loaded demo bridge",
                detail: "MockMobileCoreBridge active"
            ),
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-1_180),
                kind: .maintenance,
                title: "Planned maintenance dry-run",
                detail: "\(maintenanceRequest.operation.rawValue) - \(maintenanceRequest.connectionIDLabel) - \(maintenanceRequest.approvalStatus.rawValue)"
            ),
            AuditEvent(
                id: UUID(),
                timestamp: Date().addingTimeInterval(-1_480),
                kind: .maintenance,
                title: "Planned uninstall dry-run",
                detail: "\(uninstallMaintenanceRequest.operation.rawValue) - \(uninstallMaintenanceRequest.connectionIDLabel) - \(uninstallMaintenanceRequest.approvalStatus.rawValue)"
            )
        ]

        return DemoSnapshot(
            sessions: sessions,
            messagesBySession: messages,
            pendingCommandsBySession: [approvalID: pendingCommand],
            browserSessionsBySession: [:],
            maintenanceRequestsBySession: [
                maintenanceID: maintenanceRequest,
                uninstallMaintenanceID: uninstallMaintenanceRequest
            ],
            auditEvents: auditEvents
        )
    }

    private static let demoCredentialPlaceholder = "deepseek-mobile-demo-placeholder"

    private static func makePairingCode() -> String {
        String(format: "%06d", Int.random(in: 100_000...999_999))
    }

    private static func makeDemoPairingToken() -> String {
        "deepseek-mobile-demo-token-\(UUID().uuidString)"
    }

    private static func makeApprovalNonceSecret() -> String {
        "deepseek-mobile-approval-nonce-\(UUID().uuidString)"
    }

    private static func makeBrowserClickApprovalNonceSecret() -> String {
        "deepseek-mobile-browser-click-nonce-\(UUID().uuidString)"
    }

    private static func makeApprovalNonceLabel(for command: PendingCommand) -> String {
        "nonce-\(command.id.uuidString.prefix(8))"
    }

    private static func makeBrowserClickApprovalNonceLabel(for approval: BrowserClickApproval) -> String {
        "click-nonce-\(approval.id.uuidString.prefix(8))"
    }

    private static func makeBrowserSessionID() -> String {
        "browser-\(UUID().uuidString.prefix(8))"
    }

    private static func demoPairingAccountLabel(for mode: ConnectionMode) -> String {
        switch mode {
        case .bootstrap:
            return "Demo bootstrap pairing"
        case .ssh:
            return "Demo SSH pairing"
        case .runner:
            return "Demo runner pairing"
        case .remoteMcp:
            return "Demo remote MCP pairing"
        }
    }

    private static let documentedRunnerCapabilities = [
        RunnerCapability(name: "shell", summary: "Run approved shell commands"),
        RunnerCapability(name: "powershell", summary: "Run approved PowerShell commands"),
        RunnerCapability(name: "file", summary: "Read and write approved files"),
        RunnerCapability(name: "diagnose", summary: "Collect structured diagnostics"),
        RunnerCapability(name: "package", summary: "Inspect package manager state"),
        RunnerCapability(name: "browser", summary: "Drive browser automation"),
        RunnerCapability(name: "maintenance", summary: "Plan self-update and uninstall dry-runs"),
        RunnerCapability(name: "mcp_proxy", summary: "Proxy approved MCP tools"),
        RunnerCapability(name: "capabilities", summary: "Report runner tool metadata")
    ]

    private static func redactedBootstrapDisplayText(_ text: String) -> String {
        CoreBridgeRedactor.visibleText(text)
    }

    private static func parseRunnerCapabilityReport(from text: String) -> ParsedRunnerCapabilityReport {
        var endpoint: String?
        var accountLabel: String?
        var capabilities: [RunnerCapability] = []
        var readingCapabilities = false
        let lowercasedText = text.lowercased()
        let isOfflineFallback = lowercasedText.contains("offline:") || lowercasedText.contains("fallback:")

        for rawLine in text.components(separatedBy: .newlines) {
            let line = rawLine.trimmingCharacters(in: .whitespacesAndNewlines)
            let lowercasedLine = line.lowercased()
            guard !line.isEmpty else {
                continue
            }

            if lowercasedLine.hasPrefix("endpoint:") {
                endpoint = value(afterColonIn: line)
                readingCapabilities = false
                continue
            }

            if lowercasedLine.hasPrefix("account:") || lowercasedLine.hasPrefix("paired:") {
                accountLabel = value(afterColonIn: line)
                readingCapabilities = false
                continue
            }

            if lowercasedLine.hasPrefix("capabilities:") {
                readingCapabilities = true
                continue
            }

            guard readingCapabilities,
                  line.hasPrefix("-"),
                  let capability = parseCapabilityLine(line) else {
                continue
            }

            capabilities.append(capability)
        }

        return ParsedRunnerCapabilityReport(
            endpoint: endpoint,
            accountLabel: accountLabel,
            capabilities: capabilities,
            isOfflineFallback: isOfflineFallback
        )
    }

    private static func value(afterColonIn line: String) -> String? {
        let pieces = line.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        guard pieces.count == 2 else {
            return nil
        }

        let value = String(pieces[1]).trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? nil : value
    }

    private static func parseCapabilityLine(_ line: String) -> RunnerCapability? {
        let trimmedLine = String(line.dropFirst()).trimmingCharacters(in: .whitespacesAndNewlines)
        let pieces = trimmedLine.split(separator: ":", maxSplits: 1, omittingEmptySubsequences: false)
        let name = String(pieces[0]).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.rangeOfCharacter(from: .whitespacesAndNewlines) == nil else {
            return nil
        }

        let documentedSummary = documentedRunnerCapabilities.first { $0.name == name }?.summary
        let summary = pieces.count == 2
            ? String(pieces[1]).trimmingCharacters(in: .whitespacesAndNewlines)
            : documentedSummary

        let finalSummary = summary?.isEmpty == false ? summary ?? "" : "Reported by paired runner"
        return RunnerCapability(name: name, summary: finalSummary)
    }
}

private struct FailingPairingUpgradeProvider: CoreBridgeUniFFIJSONProvider {
    func bootstrapJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("bootstrap") }
    func auditJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("audit") }
    func browserJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("browser") }
    func maintenanceJSON() throws -> String { throw CoreBridgeJSONError.missingPayload("maintenance") }
}

private struct MockIssuedApprovalNonce {
    let rawValue: String
    let summary: ApprovalNonceSummary
}

private struct ParsedRunnerCapabilityReport {
    var endpoint: String?
    var accountLabel: String?
    var capabilities: [RunnerCapability]
    var isOfflineFallback: Bool

    var canUpgradeToRunner: Bool {
        !isOfflineFallback && !capabilities.isEmpty
    }
}

// TODO: Add a UniFFI-backed implementation here after generated Swift bindings
// are available from the Rust mobile core. Keep this protocol as the SwiftUI
// boundary so views do not import generated binding types directly.
