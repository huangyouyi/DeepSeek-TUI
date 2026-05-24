import Foundation

enum ConnectionMode: String, CaseIterable, Identifiable {
    case bootstrap = "Bootstrap"
    case ssh = "SSH"
    case runner = "Runner"
    case remoteMcp = "Remote MCP"

    var id: String { rawValue }

    var statusLabel: String {
        switch self {
        case .bootstrap:
            return "Bootstrap handoff"
        case .ssh:
            return "SSH tunnel"
        case .runner:
            return "Runner bridge"
        case .remoteMcp:
            return "Remote MCP"
        }
    }
}

enum SessionStatus: String, Identifiable {
    case ready = "Ready"
    case waitingForApproval = "Waiting for approval"
    case running = "Running"
    case disconnected = "Disconnected"

    var id: String { rawValue }
}

struct MobileSession: Identifiable, Hashable {
    let id: UUID
    var title: String
    var subtitle: String
    var status: SessionStatus
    var updatedAt: Date
    var unreadCount: Int
}

enum ChatRole: String {
    case user = "User"
    case assistant = "Assistant"
    case system = "System"
}

struct ChatMessage: Identifiable, Hashable {
    let id: UUID
    var sessionID: UUID
    var role: ChatRole
    var body: String
    var timestamp: Date
    var isThinking: Bool
}

enum CommandRisk: String {
    case low = "Low"
    case medium = "Medium"
    case high = "High"
}

struct PendingCommand: Identifiable, Hashable {
    let id: UUID
    var sessionID: UUID
    var title: String
    var command: String
    var workingDirectory: String
    var risk: CommandRisk
    var rationale: String
    var lease: CommandLeaseApproval
    var approvalNonceLabel: String
    var approvalNonceStatus: ApprovalNonceStatus
}

struct CommandLeaseApproval: Hashable {
    var leaseIDLabel: String
    var idempotencyKeyLabel: String
    var expiresAt: Date
    var approvedActionSummary: String

    var expirySummary: String {
        Self.expiryFormatter.string(from: expiresAt)
    }

    var executionConstraintSummary: String {
        "This approval allows this action to execute once before \(expirySummary)."
    }

    var approvalButtonSummary: String {
        "Approve once before \(expirySummary)"
    }

    var boundActionSummary: String {
        "One-time action: \(approvedActionSummary)"
    }

    private static let expiryFormatter: DateFormatter = {
        let formatter = DateFormatter()
        formatter.dateStyle = .medium
        formatter.timeStyle = .short
        formatter.timeZone = .current
        return formatter
    }()
}

enum BrowserSessionStatus: String, Identifiable, Hashable {
    case opened = "Opened"
    case textExtracted = "Text extracted"
    case waitingForClickApproval = "Waiting for click approval"
    case clickApproved = "Click approved"
    case clickDenied = "Click denied"

    var id: String { rawValue }
}

struct BrowserClickApproval: Identifiable, Hashable {
    let id: UUID
    var browserSessionID: String
    var targetDescription: String
    var pageURL: String
    var rationale: String
    var approvalNonceLabel: String
    var approvalNonceStatus: ApprovalNonceStatus
}

struct MockBrowserSession: Identifiable, Hashable {
    var id: String
    var sessionID: UUID
    var status: BrowserSessionStatus
    var pageTitle: String
    var pageURL: String
    var extractedTextPreview: String?
    var clickApprovalNonceLabel: String
    var clickApprovalNonceStatus: ApprovalNonceStatus
    var pendingClickApproval: BrowserClickApproval?
}

enum MaintenanceOperation: String, Hashable {
    case selfUpdate = "Self-update"
    case uninstall = "Uninstall"
}

enum MaintenanceApprovalStatus: String, Hashable {
    case planned = "Planned"
    case waitingForDryRunApproval = "Waiting for dry-run approval"
    case approved = "Dry-run approved"
    case denied = "Dry-run denied"
}

struct MaintenanceDryRunStep: Identifiable, Hashable {
    let id: UUID
    var title: String
    var detail: String
    var destructive: Bool
}

struct ArtifactVerificationPlan: Hashable {
    var artifactName: String
    var checksumSource: String
    var signatureSource: String
    var verifyStep: String
    var rollbackGuidance: String
}

struct MaintenanceApprovalRequest: Identifiable, Hashable {
    let id: UUID
    var sessionID: UUID
    var operation: MaintenanceOperation
    var title: String
    var connectionIDLabel: String
    var targetSummary: String
    var risk: CommandRisk
    var approvalStatus: MaintenanceApprovalStatus
    var dryRunSteps: [MaintenanceDryRunStep]
    var artifactVerification: ArtifactVerificationPlan?
    var auditSummary: String
    var requestedBy: String
}

enum ApprovalNonceStatus: String, Hashable {
    case notIssued = "Not issued"
    case willIssueOnApproval = "Will issue on approval"
    case issued = "Issued"
    case bound = "Bound to command"
}

struct ApprovalNonceSummary: Equatable, Hashable {
    var label: String
    var status: ApprovalNonceStatus

    var isPresent: Bool {
        status == .issued || status == .bound
    }
}

struct ConnectionSettings: Equatable {
    var mode: ConnectionMode
    var host: String
    var endpoint: String
    var port: String
    var username: String
    var tokenLabel: String
    var pendingPairingCode: String
    var pairedAccountLabel: String
    var bootstrapOutput: String
    var discoveredRunnerCapabilities: [RunnerCapability]

    static let demo = ConnectionSettings(
        mode: .bootstrap,
        host: "localhost",
        endpoint: "http://localhost:8765",
        port: "22",
        username: "mobile",
        tokenLabel: "Demo token",
        pendingPairingCode: "",
        pairedAccountLabel: "",
        bootstrapOutput: "",
        discoveredRunnerCapabilities: []
    )

    var isPaired: Bool {
        !pairedAccountLabel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var pairingStatusLabel: String {
        isPaired ? "Paired as \(pairedAccountLabel)" : "Unpaired"
    }

    var statusSummary: String {
        switch mode {
        case .bootstrap:
            return bootstrapOutput.isEmpty ? "Awaiting bootstrap output" : "Bootstrap output imported"
        case .ssh:
            return "\(username)@\(host):\(port)"
        case .runner:
            return "\(endpoint) - \(pairingStatusLabel)"
        case .remoteMcp:
            let tokenStatus = tokenLabel.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                ? "No token label"
                : "Token label configured"
            return "\(endpoint) - \(pairingStatusLabel) - \(tokenStatus)"
        }
    }
}

enum BootstrapInstallPlatform: String, CaseIterable, Identifiable, Hashable {
    case macOS = "macOS"
    case windows = "Windows"

    var id: String { rawValue }
}

struct BootstrapInstallStep: Identifiable, Hashable {
    let id: UUID
    var title: String
    var commands: [String]
    var explanation: String
    var risk: CommandRisk
    var expectedOutput: String

    var commandBlock: String {
        commands.joined(separator: "\n")
    }
}

struct BootstrapInstallGuide: Identifiable, Hashable {
    var platform: BootstrapInstallPlatform
    var title: String
    var steps: [BootstrapInstallStep]
    var sampleOutput: String
    var offlineFallbackTitle: String
    var offlineFallbackDetail: String
    var offlineFallbackExpectedOutput: String

    var id: String { platform.id }

    static let demoGuides: [BootstrapInstallGuide] = [
        BootstrapInstallGuide(
            platform: .macOS,
            title: "macOS runner bootstrap",
            steps: [
                BootstrapInstallStep(
                    id: UUID(),
                    title: "Prepare runner directory",
                    commands: [
                        "mkdir -p \"$HOME/.deepseek/runner\"",
                        "cd \"$HOME/.deepseek/runner\""
                    ],
                    explanation: "Creates the local runner workspace before the installer writes pairing metadata.",
                    risk: .low,
                    expectedOutput: "Directory exists and the shell prompt is inside ~/.deepseek/runner."
                ),
                BootstrapInstallStep(
                    id: UUID(),
                    title: "Install and print bootstrap handoff",
                    commands: [
                        "deepseek bootstrap runner install --platform macos --print-handoff"
                    ],
                    explanation: "Runs the bootstrap installer and prints the redacted handoff block for mobile import.",
                    risk: .medium,
                    expectedOutput: "deepseek-runner: installed\nhandoff: paste the output into mobile setup"
                ),
                BootstrapInstallStep(
                    id: UUID(),
                    title: "Start the local runner",
                    commands: [
                        "deepseek bootstrap runner start --foreground"
                    ],
                    explanation: "Starts the runner in the foreground so the first mobile pairing can complete.",
                    risk: .low,
                    expectedOutput: "runner listening on localhost\nwaiting for mobile pairing"
                )
            ],
            sampleOutput: """
            deepseek bootstrap handoff
            platform: macOS
            endpoint: http://127.0.0.1:8765
            runner: connected
            pairing: ready
            account: Demo macOS runner
            capabilities:
            - shell: Run approved shell commands
            - browser: Drive browser automation
            - maintenance: Plan self-update and uninstall dry-runs
            - mcp_proxy: Proxy approved MCP tools
            """,
            offlineFallbackTitle: "Offline macOS fallback",
            offlineFallbackDetail: "If the installer cannot reach the release service, keep the command output and retry after network access returns. The demo can still import a redacted handoff block for UI review.",
            offlineFallbackExpectedOutput: "offline: cached installer metadata unavailable\nfallback: save handoff request and retry later"
        ),
        BootstrapInstallGuide(
            platform: .windows,
            title: "Windows runner bootstrap",
            steps: [
                BootstrapInstallStep(
                    id: UUID(),
                    title: "Prepare runner directory",
                    commands: [
                        "New-Item -ItemType Directory -Force \"$env:LOCALAPPDATA\\DeepSeek\\runner\"",
                        "Set-Location \"$env:LOCALAPPDATA\\DeepSeek\\runner\""
                    ],
                    explanation: "Creates the Windows runner workspace under the current user's local app data.",
                    risk: .low,
                    expectedOutput: "Directory exists and PowerShell is inside the runner workspace."
                ),
                BootstrapInstallStep(
                    id: UUID(),
                    title: "Install and print bootstrap handoff",
                    commands: [
                        "deepseek.exe bootstrap runner install --platform windows --print-handoff"
                    ],
                    explanation: "Runs the Windows bootstrap installer and prints the redacted handoff block for mobile import.",
                    risk: .medium,
                    expectedOutput: "deepseek-runner: installed\r\nhandoff: paste the output into mobile setup"
                ),
                BootstrapInstallStep(
                    id: UUID(),
                    title: "Start the local runner",
                    commands: [
                        "deepseek.exe bootstrap runner start --foreground"
                    ],
                    explanation: "Starts the runner in the foreground so the first mobile pairing can complete.",
                    risk: .low,
                    expectedOutput: "runner listening on localhost\r\nwaiting for mobile pairing"
                )
            ],
            sampleOutput: """
            deepseek bootstrap handoff
            platform: Windows
            endpoint: http://127.0.0.1:8765
            runner: connected
            pairing: ready
            account: Demo Windows runner
            capabilities:
            - powershell: Run approved PowerShell commands
            - browser: Drive browser automation
            - maintenance: Plan self-update and uninstall dry-runs
            - mcp_proxy: Proxy approved MCP tools
            """,
            offlineFallbackTitle: "Offline Windows fallback",
            offlineFallbackDetail: "If PowerShell cannot reach the release service, keep the installer transcript and retry later. The demo can still import a redacted handoff block for UI review.",
            offlineFallbackExpectedOutput: "offline: release metadata unavailable\r\nfallback: save handoff request and retry later"
        )
    ]
}

enum RunnerRequestAuthAvailability: String, Hashable {
    case unavailable = "Unavailable"
    case unpaired = "Unpaired"
    case ready = "Ready"
}

struct RunnerRequestAuthMetadata: Equatable, Hashable {
    var endpoint: String
    var tokenAccountLabel: String?
    var tokenPresent: Bool
    var approvalNonce: ApprovalNonceSummary?
    var availability: RunnerRequestAuthAvailability

    var readinessLabel: String {
        availability.rawValue
    }

    var detailLabel: String {
        switch availability {
        case .unavailable:
            return "Runner request auth is unavailable for this connection mode."
        case .unpaired:
            return "Pair with this endpoint before runner requests can include auth metadata."
        case .ready:
            let label = tokenAccountLabel ?? "Paired credential"
            let tokenStatus = tokenPresent ? "token present" : "token missing"
            let nonceStatus = approvalNonce?.isPresent == true ? "approval nonce present" : "approval nonce absent"
            return "\(label) - \(tokenStatus) - \(nonceStatus)"
        }
    }

    static func unavailable(for settings: ConnectionSettings) -> RunnerRequestAuthMetadata {
        RunnerRequestAuthMetadata(
            endpoint: settings.endpoint,
            tokenAccountLabel: nil,
            tokenPresent: false,
            approvalNonce: nil,
            availability: .unavailable
        )
    }

    static func unpaired(endpoint: String) -> RunnerRequestAuthMetadata {
        RunnerRequestAuthMetadata(
            endpoint: endpoint,
            tokenAccountLabel: nil,
            tokenPresent: false,
            approvalNonce: nil,
            availability: .unpaired
        )
    }

    static func ready(
        endpoint: String,
        tokenAccountLabel: String,
        approvalNonce: ApprovalNonceSummary?
    ) -> RunnerRequestAuthMetadata {
        RunnerRequestAuthMetadata(
            endpoint: endpoint,
            tokenAccountLabel: tokenAccountLabel,
            tokenPresent: true,
            approvalNonce: approvalNonce,
            availability: .ready
        )
    }
}

struct RunnerCapability: Identifiable, Hashable, Equatable, Codable {
    var name: String
    var summary: String

    var id: String { name }
}

struct PairingUpgradeRequest: Equatable, Codable {
    var endpoint: String
    var pairingToken: String
    var capabilitiesJSON: String

    enum CodingKeys: String, CodingKey {
        case endpoint
        case pairingToken = "pairing_token"
        case capabilitiesJSON = "capabilities_json"
    }
}

struct RunnerProfile: Equatable, Codable {
    var endpoint: String
    var accountLabel: String
    var tokenLabel: String
    var capabilities: [RunnerCapability]
    var isMockFallback: Bool

    init(
        endpoint: String,
        accountLabel: String,
        tokenLabel: String,
        capabilities: [RunnerCapability],
        isMockFallback: Bool
    ) {
        self.endpoint = endpoint
        self.accountLabel = accountLabel
        self.tokenLabel = tokenLabel
        self.capabilities = capabilities
        self.isMockFallback = isMockFallback
    }

    enum CodingKeys: String, CodingKey {
        case endpoint
        case accountLabel = "account_label"
        case tokenLabel = "token_label"
        case capabilities
        case isMockFallback = "mock_fallback"
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        endpoint = try container.decode(String.self, forKey: .endpoint)
        accountLabel = try container.decode(String.self, forKey: .accountLabel)
        tokenLabel = try container.decode(String.self, forKey: .tokenLabel)
        capabilities = try container.decodeIfPresent([RunnerCapability].self, forKey: .capabilities) ?? []
        isMockFallback = try container.decodeIfPresent(Bool.self, forKey: .isMockFallback) ?? false
    }

    func runnerProfileJSON() throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(self)
        return String(decoding: data, as: UTF8.self)
    }
}

enum RunnerCapabilityKey: String, CaseIterable, Identifiable, Hashable {
    case shell
    case browser
    case maintenance
    case mcp

    var id: String { rawValue }

    var displayName: String {
        switch self {
        case .shell:
            return "Shell"
        case .browser:
            return "Browser"
        case .maintenance:
            return "Maintenance"
        case .mcp:
            return "MCP"
        }
    }

    var systemImage: String {
        switch self {
        case .shell:
            return "terminal"
        case .browser:
            return "safari"
        case .maintenance:
            return "arrow.triangle.2.circlepath"
        case .mcp:
            return "point.3.connected.trianglepath.dotted"
        }
    }

    var matchingCapabilityNames: Set<String> {
        switch self {
        case .shell:
            return ["shell", "powershell"]
        case .browser:
            return ["browser"]
        case .maintenance:
            return ["maintenance", "self_update", "self-update", "uninstall"]
        case .mcp:
            return ["mcp", "mcp_proxy", "mcp-proxy", "remote_mcp", "remote-mcp"]
        }
    }
}

struct RunnerCapabilityStatus: Identifiable, Hashable {
    var key: RunnerCapabilityKey
    var isAvailable: Bool
    var summary: String

    var id: String { key.id }

    var statusLabel: String {
        isAvailable ? "Available" : "Unavailable"
    }
}

extension Array where Element == RunnerCapability {
    func supports(_ key: RunnerCapabilityKey) -> Bool {
        contains { capability in
            key.matchingCapabilityNames.contains(capability.name.lowercased())
        }
    }

    func status(for key: RunnerCapabilityKey) -> RunnerCapabilityStatus {
        if let capability = first(where: { key.matchingCapabilityNames.contains($0.name.lowercased()) }) {
            return RunnerCapabilityStatus(key: key, isAvailable: true, summary: capability.summary)
        }

        return RunnerCapabilityStatus(
            key: key,
            isAvailable: false,
            summary: "Runner did not report this capability."
        )
    }

    var keyStatuses: [RunnerCapabilityStatus] {
        RunnerCapabilityKey.allCases.map { status(for: $0) }
    }
}

enum AuditEventKind: String {
    case connection = "Connection"
    case session = "Session"
    case command = "Command"
    case credential = "Credential"
    case bootstrap = "Bootstrap"
    case windows = "Windows"
    case browser = "Browser"
    case maintenance = "Maintenance"
}

struct AuditEvent: Identifiable, Hashable {
    let id: UUID
    var timestamp: Date
    var kind: AuditEventKind
    var title: String
    var detail: String
}

enum AuditTimelineFilter: String, CaseIterable, Identifiable {
    case all = "All"
    case maintenance = "Maintenance"
    case bootstrap = "Bootstrap"
    case windows = "Windows"
    case browser = "Browser"

    var id: String { rawValue }

    func includes(_ event: AuditEvent) -> Bool {
        switch self {
        case .all:
            return true
        case .maintenance:
            return event.kind == .maintenance
        case .bootstrap:
            return event.kind == .bootstrap
        case .windows:
            return event.kind == .windows
        case .browser:
            return event.kind == .browser
        }
    }
}

struct AuditTimelineEntry: Identifiable, Hashable {
    let event: AuditEvent
    var source: String
    var status: String
    var risk: CommandRisk
    var userAction: String
    var runnerAuditSummary: String
    var leaseLifecycleLabel: String?
    var commandLeaseMetadata: String

    var id: UUID { event.id }
    var timestamp: Date { event.timestamp }
    var title: String { event.title }
    var kind: AuditEventKind { event.kind }

    init(event: AuditEvent) {
        self.event = event
        source = Self.source(for: event)
        status = Self.status(for: event)
        risk = Self.risk(for: event)
        userAction = Self.userAction(for: event)
        runnerAuditSummary = Self.redactedSummary(for: event)
        leaseLifecycleLabel = Self.leaseLifecycleLabel(for: event)
        commandLeaseMetadata = Self.commandLeaseMetadata(for: event)
    }

    private static func source(for event: AuditEvent) -> String {
        switch event.kind {
        case .maintenance:
            return "Runner maintenance"
        case .bootstrap:
            return "Bootstrap handoff"
        case .windows:
            return "Windows rescue"
        case .browser:
            return "Browser runner"
        case .connection:
            return "Connection"
        case .session:
            return "Session"
        case .command:
            return "Command approval"
        case .credential:
            return "Credential store"
        }
    }

    private static func status(for event: AuditEvent) -> String {
        let text = "\(event.title) \(event.detail)".lowercased()
        if text.contains("denied") || text.contains("rejected") {
            return "Denied"
        }
        if text.contains("approved") || text.contains("redeemed") || text.contains("bound") || text.contains("lease accepted") || text.contains("lease consumed") {
            return "Approved"
        }
        if text.contains("offline") || text.contains("fallback") || text.contains("rescue") {
            return "Rescue"
        }
        if text.contains("waiting") || text.contains("pending") || text.contains("planned") || text.contains("nonce issued") {
            return "Waiting"
        }
        if text.contains("imported") || text.contains("opened") || text.contains("extracted") || text.contains("loaded") {
            return "Recorded"
        }
        return "Observed"
    }

    private static func risk(for event: AuditEvent) -> CommandRisk {
        let text = "\(event.title) \(event.detail)".lowercased()
        if text.contains("high risk") || text.contains("uninstall") || text.contains("destructive") {
            return .high
        }
        if text.contains("medium risk") || text.contains("install") || text.contains("bootstrap") {
            return .medium
        }
        return .low
    }

    private static func userAction(for event: AuditEvent) -> String {
        let text = "\(event.title) \(event.detail)".lowercased()
        if text.contains("denied") || text.contains("rejected") {
            return "Rejected by user"
        }
        if text.contains("nonce issued") || text.contains("lease accepted") {
            return "Review required"
        }
        if text.contains("approved") {
            return "Approved by user"
        }
        if text.contains("imported") {
            return "Imported handoff"
        }
        if text.contains("requested") || text.contains("waiting") || text.contains("pending") || text.contains("planned") {
            return "Review required"
        }
        return "No action needed"
    }

    private static func redactedSummary(for event: AuditEvent) -> String {
        var summary = event.detail
        let redactionPatterns = [
            (#"deepseek-mobile-demo-token-[A-Za-z0-9-]+"#, "redacted token"),
            (#"deepseek-mobile-approval-nonce-[A-Za-z0-9-]+"#, "redacted nonce"),
            (#"deepseek-mobile-browser-click-nonce-[A-Za-z0-9-]+"#, "redacted browser nonce"),
            (#"Bearer\s+[A-Za-z0-9._~+/\-=]+"#, "bearer redacted"),
            (#"\bpairing_token\s*=\s*("[^"]*"|'[^']*'|[^\s,;]+)"#, "pairing token redacted"),
            (#"\blease_id\s*=\s*("[^"]*"|'[^']*'|[^\s,;]+)"#, "lease id redacted"),
            (#"\blease-[A-Za-z0-9._:-]+"#, "lease-redacted"),
            (#"\bidempotency_(key|secret)\s*=\s*("[^"]*"|'[^']*'|[^\s,;]+)"#, "idempotency redacted"),
            (#"\bidem-[A-Za-z0-9._:-]+"#, "idempotency-redacted"),
            (#"\bcommand\s*=\s*("[^"]*"|'[^']*'|\{[^}]*\}|\[[^\]]*\]|[^\s,;]+)"#, "command redacted"),
            (#"\benv\s*=\s*("[^"]*"|'[^']*'|\{[^}]*\}|\[[^\]]*\]|[^\s,;]+)"#, "env redacted")
        ]
        for (pattern, redactedValue) in redactionPatterns {
            summary = summary.replacingOccurrences(
                of: pattern,
                with: redactedValue,
                options: .regularExpression
            )
        }

        let replacements = [
            "deepseek-mobile-demo-token": "redacted token",
            "deepseek-mobile-approval-nonce": "redacted nonce",
            "deepseek-mobile-browser-click-nonce": "redacted browser nonce"
        ]

        for (rawValue, redactedValue) in replacements {
            summary = summary.replacingOccurrences(of: rawValue, with: redactedValue)
        }

        if summary.lowercased().contains("nonce") {
            summary = summary.replacingOccurrences(
                of: #"nonce-[A-Za-z0-9-]+"#,
                with: "nonce-redacted",
                options: .regularExpression
            )
            summary = summary.replacingOccurrences(
                of: #"click-nonce-[A-Za-z0-9-]+"#,
                with: "click-nonce-redacted",
                options: .regularExpression
            )
        }

        return summary
    }

    private static func leaseLifecycleLabel(for event: AuditEvent) -> String? {
        let text = "\(event.title) \(event.detail)".lowercased()
        if text.contains("nonce issued") {
            return "Nonce issued"
        }
        if text.contains("lease accepted") {
            return "Lease accepted"
        }
        if text.contains("lease consumed") {
            return "Lease consumed"
        }
        if text.contains("replay") && text.contains("rejected") {
            return "Replay rejected"
        }
        if text.contains("expired") && text.contains("rejected") {
            return "Expired rejected"
        }
        if text.contains("invalid") && text.contains("rejected") {
            return "Invalid rejected"
        }
        return nil
    }

    private static func commandLeaseMetadata(for event: AuditEvent) -> String {
        let text = "\(event.title) \(event.detail)"
        let allowedKeys = ["call_id", "tool", "error_code"]
        return allowedKeys.compactMap { key in
            guard let value = firstAuditField(named: key, in: text) else {
                return nil
            }
            return "\(key)=\(value)"
        }
        .joined(separator: " - ")
    }

    private static func firstAuditField(named key: String, in text: String) -> String? {
        let pattern = #"\b\#(key)\s*=\s*([A-Za-z0-9._:-]+)"#
        guard let expression = try? NSRegularExpression(pattern: pattern) else {
            return nil
        }
        let range = NSRange(text.startIndex..<text.endIndex, in: text)
        guard let match = expression.firstMatch(in: text, range: range),
              let valueRange = Range(match.range(at: 1), in: text) else {
            return nil
        }
        return String(text[valueRange])
    }
}

enum CommandDecision {
    case approved
    case denied
}

struct DemoSnapshot {
    var sessions: [MobileSession]
    var messagesBySession: [UUID: [ChatMessage]]
    var pendingCommandsBySession: [UUID: PendingCommand]
    var browserSessionsBySession: [UUID: MockBrowserSession]
    var maintenanceRequestsBySession: [UUID: MaintenanceApprovalRequest]
    var auditEvents: [AuditEvent]
}
