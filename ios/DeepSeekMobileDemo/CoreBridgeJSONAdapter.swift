import Foundation

enum CoreBridgeUniFFICall: String, CaseIterable {
    case bootstrap
    case audit
    case browser
    case maintenance
}

protocol CoreBridgeUniFFIJSONProvider {
    func bootstrapJSON() throws -> String
    func auditJSON() throws -> String
    func browserJSON() throws -> String
    func maintenanceJSON() throws -> String
    func pairingUpgradeProfileJSON(requestJSON: String) throws -> String
}

extension CoreBridgeUniFFIJSONProvider {
    func pairingUpgradeProfileJSON(requestJSON: String) throws -> String {
        throw CoreBridgeJSONError.missingPayload("pairing_upgrade")
    }
}

typealias CoreBridgeJSONAPI = CoreBridgeUniFFIJSONProvider

enum CoreBridgeJSONError: Error, Equatable {
    case missingPayload(String)
    case invalidPayload(String)
}

struct CoreBridgeUniFFICallSite {
    private let provider: CoreBridgeUniFFIJSONProvider

    init(provider: CoreBridgeUniFFIJSONProvider) {
        self.provider = provider
    }

    func bootstrapJSON() throws -> String {
        try json(for: .bootstrap)
    }

    func auditJSON() throws -> String {
        try json(for: .audit)
    }

    func browserJSON() throws -> String {
        try json(for: .browser)
    }

    func maintenanceJSON() throws -> String {
        try json(for: .maintenance)
    }

    func pairingUpgradeProfileJSON(requestJSON: String) throws -> String {
        try provider.pairingUpgradeProfileJSON(requestJSON: requestJSON)
    }

    private func json(for call: CoreBridgeUniFFICall) throws -> String {
        switch call {
        case .bootstrap:
            return try provider.bootstrapJSON()
        case .audit:
            return try provider.auditJSON()
        case .browser:
            return try provider.browserJSON()
        case .maintenance:
            return try provider.maintenanceJSON()
        }
    }
}

struct MockCoreBridgeJSONAPI: CoreBridgeUniFFIJSONProvider {
    var bootstrapPayload: String
    var auditPayload: String
    var browserPayload: String
    var maintenancePayload: String

    func bootstrapJSON() throws -> String { bootstrapPayload }
    func auditJSON() throws -> String { auditPayload }
    func browserJSON() throws -> String { browserPayload }
    func maintenanceJSON() throws -> String { maintenancePayload }
}

struct CoreBridgeInitialState {
    var snapshot: DemoSnapshot
    var connectionSettings: ConnectionSettings
}

struct CoreBridgeJSONAdapter {
    private let callSite: CoreBridgeUniFFICallSite
    private let decoder: JSONDecoder

    init(provider: CoreBridgeUniFFIJSONProvider) {
        callSite = CoreBridgeUniFFICallSite(provider: provider)
        decoder = JSONDecoder()
    }

    init(api: CoreBridgeJSONAPI) {
        callSite = CoreBridgeUniFFICallSite(provider: api)
        decoder = JSONDecoder()
    }

    func initialState(fallbackSnapshot: DemoSnapshot, fallbackSettings: ConnectionSettings) throws -> CoreBridgeInitialState {
        var snapshot = fallbackSnapshot
        var settings = fallbackSettings

        let bootstrap = try decode(CoreBootstrapPayload.self, from: callSite.bootstrapJSON(), payloadName: "bootstrap")
        let audit = try decode(CoreAuditPayload.self, from: callSite.auditJSON(), payloadName: "audit")
        let browser = try decode(CoreBrowserPayload.self, from: callSite.browserJSON(), payloadName: "browser")
        let maintenance = try decode(CoreMaintenancePayload.self, from: callSite.maintenanceJSON(), payloadName: "maintenance")

        if let connection = bootstrap.connection {
            settings = connection.connectionSettings(fallback: settings)
        }
        if let sessions = bootstrap.sessions, !sessions.isEmpty {
            snapshot.sessions = sessions.map(\.mobileSession)
        }
        if let messages = bootstrap.messages {
            snapshot.messagesBySession = Dictionary(grouping: messages.map(\.chatMessage), by: \.sessionID)
        }

        snapshot.auditEvents = audit.events.map(\.auditEvent)
        snapshot.browserSessionsBySession = Dictionary(
            uniqueKeysWithValues: browser.sessions.compactMap { session in
                guard let browserSession = session.browserSession else {
                    return nil
                }
                return (browserSession.sessionID, browserSession)
            }
        )
        snapshot.maintenanceRequestsBySession = Dictionary(
            uniqueKeysWithValues: maintenance.requests.compactMap { request in
                guard let approval = request.maintenanceApprovalRequest else {
                    return nil
                }
                return (approval.sessionID, approval)
            }
        )

        return CoreBridgeInitialState(snapshot: snapshot, connectionSettings: settings)
    }

    func pairingUpgradeProfile(endpoint: String, pairingToken: String, capabilitiesJSON: String) -> RunnerProfile {
        let request = PairingUpgradeRequest(
            endpoint: endpoint,
            pairingToken: pairingToken,
            capabilitiesJSON: capabilitiesJSON
        )

        do {
            let requestJSON = try encode(request)
            let profileJSON = try callSite.pairingUpgradeProfileJSON(requestJSON: requestJSON)
            let profile = try decode(RunnerProfile.self, from: profileJSON, payloadName: "pairing_upgrade")
            return sanitized(profile, redacting: pairingToken)
        } catch {
            return mockPairingUpgradeProfile(
                endpoint: endpoint,
                pairingToken: pairingToken,
                capabilitiesJSON: capabilitiesJSON
            )
        }
    }

    func pairingUpgradeRunnerProfileJSON(endpoint: String, pairingToken: String, capabilitiesJSON: String) throws -> String {
        try pairingUpgradeProfile(
            endpoint: endpoint,
            pairingToken: pairingToken,
            capabilitiesJSON: capabilitiesJSON
        ).runnerProfileJSON()
    }

    private func decode<T: Decodable>(_ type: T.Type, from text: String, payloadName: String) throws -> T {
        guard !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            throw CoreBridgeJSONError.missingPayload(payloadName)
        }

        guard let data = text.data(using: .utf8) else {
            throw CoreBridgeJSONError.invalidPayload(payloadName)
        }

        do {
            return try decoder.decode(type, from: data)
        } catch {
            throw CoreBridgeJSONError.invalidPayload(payloadName)
        }
    }

    private func encode<T: Encodable>(_ value: T) throws -> String {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys]
        let data = try encoder.encode(value)
        return String(decoding: data, as: UTF8.self)
    }

    private func sanitized(_ profile: RunnerProfile, redacting pairingToken: String) -> RunnerProfile {
        RunnerProfile(
            endpoint: CoreBridgeRedactor.visibleText(profile.endpoint, redacting: [pairingToken]),
            accountLabel: CoreBridgeRedactor.visibleText(profile.accountLabel, redacting: [pairingToken]),
            tokenLabel: CoreBridgeRedactor.visibleText(profile.tokenLabel, redacting: [pairingToken]),
            capabilities: profile.capabilities.map { capability in
                RunnerCapability(
                    name: CoreBridgeRedactor.visibleText(capability.name, redacting: [pairingToken]),
                    summary: CoreBridgeRedactor.visibleText(capability.summary, redacting: [pairingToken])
                )
            },
            isMockFallback: profile.isMockFallback
        )
    }

    private func mockPairingUpgradeProfile(
        endpoint: String,
        pairingToken: String,
        capabilitiesJSON: String
    ) -> RunnerProfile {
        RunnerProfile(
            endpoint: CoreBridgeRedactor.visibleText(endpoint, redacting: [pairingToken]),
            accountLabel: "Mock upgraded runner",
            tokenLabel: "Mock pairing credential",
            capabilities: CorePairingCapabilitiesDTO.capabilities(from: capabilitiesJSON, redacting: pairingToken),
            isMockFallback: true
        )
    }
}

enum CoreBridgeRedactor {
    static func visibleText(_ text: String) -> String {
        visibleText(text, redacting: [])
    }

    static func visibleText(_ text: String, redacting secrets: [String]) -> String {
        let tokenRedactedText = secrets.reduce(text) { result, secret in
            let trimmedSecret = secret.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !trimmedSecret.isEmpty else {
                return result
            }
            return result.replacingOccurrences(of: trimmedSecret, with: "redacted pairing token")
        }

        return tokenRedactedText
            .components(separatedBy: .newlines)
            .map(redactedLine)
            .joined(separator: "\n")
    }

    private static func redactedLine(_ line: String) -> String {
        let lowercased = line.lowercased()
        if lowercased.contains("token:") ||
            lowercased.contains("secret:") ||
            lowercased.contains("nonce:") {
            let prefix = line.split(separator: ":", maxSplits: 1).first.map(String.init) ?? "secret"
            return "\(prefix): redacted by demo"
        }

        var result = line
        let redactionPatterns = [
            (#"deepseek-mobile-demo-token-[A-Za-z0-9-]+"#, "redacted token"),
            (#"deepseek-mobile-approval-nonce-[A-Za-z0-9-]+"#, "redacted nonce"),
            (#"deepseek-mobile-browser-click-nonce-[A-Za-z0-9-]+"#, "redacted browser nonce"),
            (#"nonce-[A-Za-z0-9-]+"#, "nonce-redacted"),
            (#"click-nonce-[A-Za-z0-9-]+"#, "click-nonce-redacted")
        ]
        for (pattern, replacement) in redactionPatterns {
            result = result.replacingOccurrences(
                of: pattern,
                with: replacement,
                options: .regularExpression
            )
        }

        return result
    }
}
private struct CoreBootstrapPayload: Decodable {
    var connection: CoreConnectionDTO?
    var sessions: [CoreSessionDTO]?
    var messages: [CoreMessageDTO]?
}

private struct CoreAuditPayload: Decodable {
    var events: [CoreAuditEventDTO]
}

private struct CoreBrowserPayload: Decodable {
    var sessions: [CoreBrowserSessionDTO]
}

private struct CoreMaintenancePayload: Decodable {
    var requests: [CoreMaintenanceRequestDTO]
}

private struct CorePairingCapabilitiesDTO: Decodable {
    var capabilities: [CorePairingCapabilityDTO]

    static func capabilities(from text: String, redacting pairingToken: String) -> [RunnerCapability] {
        guard let data = text.data(using: .utf8) else {
            return []
        }

        let decoder = JSONDecoder()
        if let envelope = try? decoder.decode(CorePairingCapabilitiesDTO.self, from: data) {
            return envelope.capabilities.map { $0.runnerCapability(redacting: pairingToken) }
        }

        if let capabilities = try? decoder.decode([CorePairingCapabilityDTO].self, from: data) {
            return capabilities.map { $0.runnerCapability(redacting: pairingToken) }
        }

        return []
    }
}

private struct CorePairingCapabilityDTO: Decodable {
    var name: String
    var summary: String?

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let name = try? container.decode(String.self) {
            self.name = name
            self.summary = nil
            return
        }

        let object = try decoder.container(keyedBy: CodingKeys.self)
        name = try object.decode(String.self, forKey: .name)
        summary = try object.decodeIfPresent(String.self, forKey: .summary)
    }

    private enum CodingKeys: String, CodingKey {
        case name
        case summary
    }

    func runnerCapability(redacting pairingToken: String) -> RunnerCapability {
        RunnerCapability(
            name: CoreBridgeRedactor.visibleText(name, redacting: [pairingToken]),
            summary: CoreBridgeRedactor.visibleText(summary ?? "Reported by paired runner", redacting: [pairingToken])
        )
    }
}

private struct CoreConnectionDTO: Decodable {
    var mode: String?
    var endpoint: String?
    var host: String?
    var port: String?
    var username: String?
    var accountLabel: String?
    var tokenLabel: String?
    var bootstrapOutput: String?
    var capabilities: [CoreRunnerCapabilityDTO]?

    enum CodingKeys: String, CodingKey {
        case mode
        case endpoint
        case host
        case port
        case username
        case accountLabel = "account_label"
        case tokenLabel = "token_label"
        case bootstrapOutput = "bootstrap_output"
        case capabilities
    }

    func connectionSettings(fallback: ConnectionSettings) -> ConnectionSettings {
        var settings = fallback
        if let mode {
            settings.mode = ConnectionMode(coreValue: mode) ?? settings.mode
        }
        settings.endpoint = endpoint ?? settings.endpoint
        settings.host = host ?? settings.host
        settings.port = port ?? settings.port
        settings.username = username ?? settings.username
        settings.pairedAccountLabel = CoreBridgeRedactor.visibleText(accountLabel ?? settings.pairedAccountLabel)
        settings.tokenLabel = CoreBridgeRedactor.visibleText(tokenLabel ?? accountLabel ?? settings.tokenLabel)
        settings.bootstrapOutput = CoreBridgeRedactor.visibleText(bootstrapOutput ?? settings.bootstrapOutput)
        settings.discoveredRunnerCapabilities = capabilities?.map(\.runnerCapability) ?? settings.discoveredRunnerCapabilities
        return settings
    }
}

private struct CoreRunnerCapabilityDTO: Decodable {
    var name: String
    var summary: String?

    var runnerCapability: RunnerCapability {
        RunnerCapability(name: name, summary: summary ?? "Reported by paired runner")
    }
}

private struct CoreSessionDTO: Decodable {
    var id: UUID
    var title: String
    var subtitle: String?
    var status: String?
    var updatedAtUnix: TimeInterval?
    var unreadCount: Int?

    enum CodingKeys: String, CodingKey {
        case id
        case title
        case subtitle
        case status
        case updatedAtUnix = "updated_at_unix"
        case unreadCount = "unread_count"
    }

    var mobileSession: MobileSession {
        MobileSession(
            id: id,
            title: CoreBridgeRedactor.visibleText(title),
            subtitle: CoreBridgeRedactor.visibleText(subtitle ?? "Core bridge session"),
            status: SessionStatus(coreValue: status) ?? .ready,
            updatedAt: Date(timeIntervalSince1970: updatedAtUnix ?? Date().timeIntervalSince1970),
            unreadCount: unreadCount ?? 0
        )
    }
}

private struct CoreMessageDTO: Decodable {
    var id: UUID
    var sessionID: UUID
    var role: String
    var body: String
    var timestampUnix: TimeInterval?
    var isThinking: Bool?

    enum CodingKeys: String, CodingKey {
        case id
        case sessionID = "session_id"
        case role
        case body
        case timestampUnix = "timestamp_unix"
        case isThinking = "is_thinking"
    }

    var chatMessage: ChatMessage {
        ChatMessage(
            id: id,
            sessionID: sessionID,
            role: ChatRole(coreValue: role) ?? .assistant,
            body: CoreBridgeRedactor.visibleText(body),
            timestamp: Date(timeIntervalSince1970: timestampUnix ?? Date().timeIntervalSince1970),
            isThinking: isThinking ?? false
        )
    }
}

private struct CoreAuditEventDTO: Decodable {
    var id: UUID
    var timestampUnix: TimeInterval?
    var kind: String
    var title: String
    var detail: String

    enum CodingKeys: String, CodingKey {
        case id
        case timestampUnix = "timestamp_unix"
        case kind
        case title
        case detail
    }

    var auditEvent: AuditEvent {
        AuditEvent(
            id: id,
            timestamp: Date(timeIntervalSince1970: timestampUnix ?? Date().timeIntervalSince1970),
            kind: AuditEventKind(coreValue: kind) ?? .connection,
            title: CoreBridgeRedactor.visibleText(title),
            detail: CoreBridgeRedactor.visibleText(detail)
        )
    }
}

private struct CoreBrowserSessionDTO: Decodable {
    var appSessionID: UUID
    var browserSessionID: String
    var status: String?
    var pageTitle: String
    var pageURL: String
    var extractedTextPreview: String?
    var clickApprovalNonceLabel: String?
    var clickApprovalNonceStatus: String?
    var pendingClickApproval: CoreBrowserClickApprovalDTO?

    enum CodingKeys: String, CodingKey {
        case appSessionID = "app_session_id"
        case browserSessionID = "browser_session_id"
        case status
        case pageTitle = "page_title"
        case pageURL = "page_url"
        case extractedTextPreview = "extracted_text_preview"
        case clickApprovalNonceLabel = "click_approval_nonce_label"
        case clickApprovalNonceStatus = "click_approval_nonce_status"
        case pendingClickApproval = "pending_click_approval"
    }

    var browserSession: MockBrowserSession? {
        MockBrowserSession(
            id: CoreBridgeRedactor.visibleText(browserSessionID),
            sessionID: appSessionID,
            status: BrowserSessionStatus(coreValue: status) ?? .opened,
            pageTitle: CoreBridgeRedactor.visibleText(pageTitle),
            pageURL: CoreBridgeRedactor.visibleText(pageURL),
            extractedTextPreview: extractedTextPreview.map(CoreBridgeRedactor.visibleText),
            clickApprovalNonceLabel: CoreBridgeRedactor.visibleText(clickApprovalNonceLabel ?? "Not issued"),
            clickApprovalNonceStatus: ApprovalNonceStatus(coreValue: clickApprovalNonceStatus) ?? .notIssued,
            pendingClickApproval: pendingClickApproval?.browserClickApproval(browserSessionID: browserSessionID)
        )
    }
}

private struct CoreBrowserClickApprovalDTO: Decodable {
    var id: UUID
    var targetDescription: String
    var pageURL: String
    var rationale: String
    var approvalNonceLabel: String?
    var approvalNonceStatus: String?

    enum CodingKeys: String, CodingKey {
        case id
        case targetDescription = "target_description"
        case pageURL = "page_url"
        case rationale
        case approvalNonceLabel = "approval_nonce_label"
        case approvalNonceStatus = "approval_nonce_status"
    }

    func browserClickApproval(browserSessionID: String) -> BrowserClickApproval {
        BrowserClickApproval(
            id: id,
            browserSessionID: CoreBridgeRedactor.visibleText(browserSessionID),
            targetDescription: CoreBridgeRedactor.visibleText(targetDescription),
            pageURL: CoreBridgeRedactor.visibleText(pageURL),
            rationale: CoreBridgeRedactor.visibleText(rationale),
            approvalNonceLabel: CoreBridgeRedactor.visibleText(approvalNonceLabel ?? "Issued only after browser click approval"),
            approvalNonceStatus: ApprovalNonceStatus(coreValue: approvalNonceStatus) ?? .willIssueOnApproval
        )
    }
}

private struct CoreMaintenanceRequestDTO: Decodable {
    var id: UUID
    var appSessionID: UUID
    var operation: String
    var title: String
    var connectionIDLabel: String
    var targetSummary: String
    var risk: String
    var approvalStatus: String
    var dryRunSteps: [CoreMaintenanceStepDTO]
    var artifactVerification: CoreArtifactVerificationDTO?
    var auditSummary: String
    var requestedBy: String

    enum CodingKeys: String, CodingKey {
        case id
        case appSessionID = "app_session_id"
        case operation
        case title
        case connectionIDLabel = "connection_id_label"
        case targetSummary = "target_summary"
        case risk
        case approvalStatus = "approval_status"
        case dryRunSteps = "dry_run_steps"
        case artifactVerification = "artifact_verification"
        case auditSummary = "audit_summary"
        case requestedBy = "requested_by"
    }

    var maintenanceApprovalRequest: MaintenanceApprovalRequest? {
        guard let operation = MaintenanceOperation(coreValue: operation) else {
            return nil
        }

        return MaintenanceApprovalRequest(
            id: id,
            sessionID: appSessionID,
            operation: operation,
            title: CoreBridgeRedactor.visibleText(title),
            connectionIDLabel: CoreBridgeRedactor.visibleText(connectionIDLabel),
            targetSummary: CoreBridgeRedactor.visibleText(targetSummary),
            risk: CommandRisk(coreValue: risk) ?? .medium,
            approvalStatus: MaintenanceApprovalStatus(coreValue: approvalStatus) ?? .planned,
            dryRunSteps: dryRunSteps.map(\.maintenanceDryRunStep),
            artifactVerification: artifactVerification?.artifactVerificationPlan,
            auditSummary: CoreBridgeRedactor.visibleText(auditSummary),
            requestedBy: CoreBridgeRedactor.visibleText(requestedBy)
        )
    }
}

private struct CoreMaintenanceStepDTO: Decodable {
    var id: UUID
    var title: String
    var detail: String
    var destructive: Bool

    var maintenanceDryRunStep: MaintenanceDryRunStep {
        MaintenanceDryRunStep(
            id: id,
            title: CoreBridgeRedactor.visibleText(title),
            detail: CoreBridgeRedactor.visibleText(detail),
            destructive: destructive
        )
    }
}

private struct CoreArtifactVerificationDTO: Decodable {
    var artifactName: String
    var checksumSource: String
    var signatureSource: String
    var verifyStep: String
    var rollbackGuidance: String

    enum CodingKeys: String, CodingKey {
        case artifactName = "artifact_name"
        case checksumSource = "checksum_source"
        case signatureSource = "signature_source"
        case verifyStep = "verify_step"
        case rollbackGuidance = "rollback_guidance"
    }

    var artifactVerificationPlan: ArtifactVerificationPlan {
        ArtifactVerificationPlan(
            artifactName: CoreBridgeRedactor.visibleText(artifactName),
            checksumSource: CoreBridgeRedactor.visibleText(checksumSource),
            signatureSource: CoreBridgeRedactor.visibleText(signatureSource),
            verifyStep: CoreBridgeRedactor.visibleText(verifyStep),
            rollbackGuidance: CoreBridgeRedactor.visibleText(rollbackGuidance)
        )
    }
}

private extension ConnectionMode {
    init?(coreValue: String) {
        switch coreValue.normalizedCoreEnumValue {
        case "bootstrap":
            self = .bootstrap
        case "ssh":
            self = .ssh
        case "runner":
            self = .runner
        case "remote_mcp", "remotemcp":
            self = .remoteMcp
        default:
            return nil
        }
    }
}

private extension SessionStatus {
    init?(coreValue: String?) {
        switch coreValue?.normalizedCoreEnumValue {
        case "ready":
            self = .ready
        case "waiting_for_approval", "waitingforapproval":
            self = .waitingForApproval
        case "running":
            self = .running
        case "disconnected":
            self = .disconnected
        default:
            return nil
        }
    }
}

private extension ChatRole {
    init?(coreValue: String) {
        switch coreValue.normalizedCoreEnumValue {
        case "user":
            self = .user
        case "assistant":
            self = .assistant
        case "system":
            self = .system
        default:
            return nil
        }
    }
}

private extension AuditEventKind {
    init?(coreValue: String) {
        switch coreValue.normalizedCoreEnumValue {
        case "connection":
            self = .connection
        case "session":
            self = .session
        case "command":
            self = .command
        case "credential":
            self = .credential
        case "bootstrap":
            self = .bootstrap
        case "windows":
            self = .windows
        case "browser":
            self = .browser
        case "maintenance":
            self = .maintenance
        default:
            return nil
        }
    }
}

private extension BrowserSessionStatus {
    init?(coreValue: String?) {
        switch coreValue?.normalizedCoreEnumValue {
        case "opened":
            self = .opened
        case "text_extracted", "textextracted":
            self = .textExtracted
        case "waiting_for_click_approval", "waitingforclickapproval":
            self = .waitingForClickApproval
        case "click_approved", "clickapproved":
            self = .clickApproved
        case "click_denied", "clickdenied":
            self = .clickDenied
        default:
            return nil
        }
    }
}

private extension ApprovalNonceStatus {
    init?(coreValue: String?) {
        switch coreValue?.normalizedCoreEnumValue {
        case "not_issued", "notissued":
            self = .notIssued
        case "will_issue_on_approval", "willissueonapproval":
            self = .willIssueOnApproval
        case "issued":
            self = .issued
        case "bound", "bound_to_command", "boundtocommand":
            self = .bound
        default:
            return nil
        }
    }
}

private extension MaintenanceOperation {
    init?(coreValue: String) {
        switch coreValue.normalizedCoreEnumValue {
        case "self_update", "selfupdate":
            self = .selfUpdate
        case "uninstall":
            self = .uninstall
        default:
            return nil
        }
    }
}

private extension MaintenanceApprovalStatus {
    init?(coreValue: String) {
        switch coreValue.normalizedCoreEnumValue {
        case "planned":
            self = .planned
        case "waiting_for_dry_run_approval", "waitingfordryrunapproval":
            self = .waitingForDryRunApproval
        case "approved", "dry_run_approved", "dryrunapproved":
            self = .approved
        case "denied", "dry_run_denied", "dryrundenied":
            self = .denied
        default:
            return nil
        }
    }
}

private extension CommandRisk {
    init?(coreValue: String) {
        switch coreValue.normalizedCoreEnumValue {
        case "low":
            self = .low
        case "medium":
            self = .medium
        case "high":
            self = .high
        default:
            return nil
        }
    }
}

private extension String {
    var normalizedCoreEnumValue: String {
        trimmingCharacters(in: .whitespacesAndNewlines)
            .lowercased()
            .replacingOccurrences(of: "-", with: "_")
            .replacingOccurrences(of: " ", with: "_")
    }
}
