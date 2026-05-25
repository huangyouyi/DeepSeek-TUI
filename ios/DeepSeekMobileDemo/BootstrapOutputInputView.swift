import SwiftUI
#if canImport(UIKit)
import UIKit
#endif

struct BootstrapOutputInputView: View {
    @Environment(\.dismiss) private var dismiss
    @EnvironmentObject private var bridge: MockMobileCoreBridge
    @State private var bootstrapOutput = ""
    @State private var selectedPlatform: BootstrapInstallPlatform = .macOS
    @State private var copiedStepID: UUID?
    @State private var offlineFallbackEnabled = false
    @State private var importMessage = ""

    private let installGuides = BootstrapInstallGuide.demoGuides

    var body: some View {
        NavigationStack {
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    Picker("Platform", selection: $selectedPlatform) {
                        ForEach(BootstrapInstallPlatform.allCases) { platform in
                            Text(platform.rawValue).tag(platform)
                        }
                    }
                    .pickerStyle(.segmented)

                    flowStatusSection
                    installStepsSection
                    pasteOutputSection
                    offlineFallbackSection

                    Spacer(minLength: 0)
                }
                .padding()
            }
            .navigationTitle("Bootstrap")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button("Cancel") {
                        dismiss()
                    }
                }

                ToolbarItem(placement: .confirmationAction) {
                    Button("Import") {
                        bridge.importBootstrapOutput(bootstrapOutput)
                        importMessage = importedStateMessage
                    }
                    .disabled(bootstrapOutput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
                }
            }
            .onAppear {
                bootstrapOutput = bridge.connectionSettings.bootstrapOutput
            }
            .onChange(of: bootstrapOutput) { newValue in
                let redacted = redactedBootstrapText(newValue)
                if redacted != newValue {
                    bootstrapOutput = redacted
                }
            }
        }
    }

    private var selectedGuide: BootstrapInstallGuide {
        installGuides.first(where: { $0.platform == selectedPlatform }) ?? installGuides[0]
    }

    private var installStepsSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(selectedGuide.title)
                .font(.headline)

            ForEach(selectedGuide.steps) { step in
                BootstrapInstallStepCard(
                    step: step,
                    copyStateLabel: copiedStepID == step.id ? "Copied" : "Copy",
                    onCopy: {
                        copyCommandBlock(step.commandBlock)
                        copiedStepID = step.id
                    }
                )
            }
        }
    }

    private var flowStatusSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("Bootstrap to runner")
                .font(.headline)

            VStack(alignment: .leading, spacing: 10) {
                BootstrapFlowRow(
                    title: "Copy bootstrap command",
                    detail: copiedStepID == nil ? "Choose the installer command for this platform." : "Command copied for the host terminal.",
                    state: copiedStepID == nil ? .active : .complete
                )
                BootstrapFlowRow(
                    title: "Paste runner capability report",
                    detail: bootstrapOutput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
                        ? "Import waits for the redacted handoff output."
                        : pasteReportDetail,
                    state: bootstrapOutput.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty ? .pending : .complete
                )
                BootstrapFlowRow(
                    title: "Enable runner tools",
                    detail: runnerToolsDetail,
                    state: runnerToolsEnabled ? .complete : .pending
                )
            }

            if !importMessage.isEmpty {
                Label(importMessage, systemImage: runnerToolsEnabled ? "checkmark.seal" : "info.circle")
                    .font(.caption)
                    .foregroundStyle(runnerToolsEnabled ? Color.green : Color.secondary)
            }
        }
        .padding(12)
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var pasteOutputSection: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text("Paste runner capability report")
                    .font(.headline)

                Spacer()

                Button {
                    bootstrapOutput = redactedBootstrapText(selectedGuide.sampleOutput)
                    importMessage = ""
                } label: {
                    Label("Mock Paste", systemImage: "doc.on.clipboard")
                }
                .buttonStyle(.bordered)
            }

            TextEditor(text: $bootstrapOutput)
                .font(.system(.body, design: .monospaced))
                .frame(minHeight: 180)
                .padding(8)
                .overlay {
                    RoundedRectangle(cornerRadius: 8, style: .continuous)
                        .stroke(Color.secondary.opacity(0.25))
                }

            HStack {
                Text("\(bootstrapOutput.count) characters")
                Spacer()
                Text("Raw token and nonce values are not displayed.")
            }
            .font(.caption)
            .foregroundStyle(.secondary)
        }
    }

    private var offlineFallbackSection: some View {
        VStack(alignment: .leading, spacing: 10) {
            Toggle(isOn: $offlineFallbackEnabled) {
                Label("Offline fallback", systemImage: offlineFallbackEnabled ? "wifi.slash" : "wifi")
            }

            if offlineFallbackEnabled {
                VStack(alignment: .leading, spacing: 8) {
                    Text(selectedGuide.offlineFallbackTitle)
                        .font(.subheadline.weight(.semibold))

                    Text(selectedGuide.offlineFallbackDetail)
                        .font(.caption)
                        .foregroundStyle(.secondary)

                    Text(selectedGuide.offlineFallbackExpectedOutput)
                        .font(.system(.caption, design: .monospaced))
                        .textSelection(.enabled)
                        .padding(10)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(Color.secondary.opacity(0.08))
                        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

                    Button {
                        bootstrapOutput = redactedBootstrapText(selectedGuide.offlineFallbackExpectedOutput)
                        importMessage = ""
                    } label: {
                        Label("Use Fallback Text", systemImage: "tray.and.arrow.down")
                            .frame(maxWidth: .infinity)
                    }
                    .buttonStyle(.bordered)
                }
            } else {
                Label("Online installer path selected", systemImage: "checkmark.circle")
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
        .padding(12)
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var runnerToolsEnabled: Bool {
        bridge.connectionSettings.mode == .runner &&
            !bridge.connectionSettings.discoveredRunnerCapabilities.isEmpty
    }

    private var pasteReportDetail: String {
        if isOfflineFallbackText {
            return "Offline fallback text is ready to import, but it will stay in Bootstrap."
        }

        if bootstrapOutput.lowercased().contains("capabilities:") {
            return "Capability report ready; import can switch to Runner."
        }

        return "Bootstrap text ready; runner tools need a capability report."
    }

    private var runnerToolsDetail: String {
        if runnerToolsEnabled {
            let count = bridge.connectionSettings.discoveredRunnerCapabilities.count
            return "\(count) capabilities available from \(bridge.connectionSettings.endpoint)."
        }

        if bridge.connectionSettings.mode == .bootstrap {
            return "Still in Bootstrap; runner tools remain disabled."
        }

        return "Waiting for runner capabilities."
    }

    private var importedStateMessage: String {
        if runnerToolsEnabled {
            return "Runner mode enabled from capability report."
        }

        if isOfflineFallbackText {
            return "Offline fallback imported; Bootstrap mode retained."
        }

        return "Bootstrap output imported; no runner capability report found."
    }

    private var isOfflineFallbackText: Bool {
        let lowercased = bootstrapOutput.lowercased()
        return lowercased.contains("offline:") || lowercased.contains("fallback:")
    }

    private func copyCommandBlock(_ commandBlock: String) {
        #if canImport(UIKit)
        UIPasteboard.general.string = commandBlock
        #endif
    }

    private func redactedBootstrapText(_ text: String) -> String {
        text
            .components(separatedBy: .newlines)
            .map { line in
                let lowercased = line.lowercased()
                if lowercased.contains("token:") ||
                    lowercased.contains("secret:") ||
                    lowercased.contains("nonce:") {
                    let prefix = line.split(separator: ":", maxSplits: 1).first.map(String.init) ?? "secret"
                    return "\(prefix): redacted by demo"
                }

                return line
                    .replacingOccurrences(
                        of: #"deepseek-mobile-demo-token-[A-Za-z0-9-]+"#,
                        with: "redacted token",
                        options: .regularExpression
                    )
                    .replacingOccurrences(
                        of: #"deepseek-mobile-(approval|browser-click)-nonce-[A-Za-z0-9-]+"#,
                        with: "redacted nonce",
                        options: .regularExpression
                    )
            }
            .joined(separator: "\n")
    }
}

private enum BootstrapFlowState {
    case pending
    case active
    case complete
}

private struct BootstrapFlowRow: View {
    let title: String
    let detail: String
    let state: BootstrapFlowState

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: systemImage)
                .foregroundStyle(color)
                .frame(width: 20)

            VStack(alignment: .leading, spacing: 2) {
                Text(title)
                    .font(.subheadline.weight(.semibold))
                Text(detail)
                    .font(.caption)
                    .foregroundStyle(.secondary)
            }
        }
    }

    private var systemImage: String {
        switch state {
        case .pending:
            return "circle"
        case .active:
            return "arrow.right.circle"
        case .complete:
            return "checkmark.circle.fill"
        }
    }

    private var color: Color {
        switch state {
        case .pending:
            return .secondary
        case .active:
            return .orange
        case .complete:
            return .green
        }
    }
}

private struct BootstrapInstallStepCard: View {
    let step: BootstrapInstallStep
    let copyStateLabel: String
    let onCopy: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack(alignment: .top) {
                Label(step.title, systemImage: "terminal")
                    .font(.subheadline.weight(.semibold))

                Spacer()

                Text(step.risk.rawValue)
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(riskColor)
            }

            Text(step.commandBlock)
                .font(.system(.caption, design: .monospaced))
                .textSelection(.enabled)
                .padding(10)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(Color.secondary.opacity(0.08))
                .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))

            Text(step.explanation)
                .font(.caption)
                .foregroundStyle(.secondary)

            VStack(alignment: .leading, spacing: 4) {
                Text("Expected output")
                    .font(.caption.weight(.semibold))
                    .foregroundStyle(.secondary)
                Text(step.expectedOutput)
                    .font(.system(.caption, design: .monospaced))
                    .textSelection(.enabled)
            }

            Button {
                onCopy()
            } label: {
                Label(copyStateLabel, systemImage: copyStateLabel == "Copied" ? "checkmark" : "doc.on.doc")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.bordered)
        }
        .padding(12)
        .background(Color(.secondarySystemBackground))
        .clipShape(RoundedRectangle(cornerRadius: 8, style: .continuous))
    }

    private var riskColor: Color {
        switch step.risk {
        case .low:
            return .green
        case .medium:
            return .orange
        case .high:
            return .red
        }
    }
}
