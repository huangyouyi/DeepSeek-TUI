#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootstrapStep {
    pub step_id: String,
    pub title: String,
    pub commands: Vec<String>,
    pub instruction: String,
    pub command: String,
    pub explanation: String,
    pub risk: String,
    pub expected_output: String,
    pub approval_required: bool,
    pub submitted_output: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct BootstrapSession {
    pub steps: Vec<BootstrapStep>,
}

struct BootstrapStepSpec<'a> {
    step_id: &'a str,
    title: &'a str,
    commands: Vec<String>,
    instruction: &'a str,
    explanation: &'a str,
    risk: &'a str,
    expected_output: &'a str,
    approval_required: bool,
}

impl BootstrapSession {
    pub fn homebrew_bootstrap_step(&mut self) -> BootstrapStep {
        let command = [
            "uname -a",
            "sw_vers",
            "command -v brew",
            "xcode-select -p",
            "echo \"$PATH\"",
        ]
        .join("\n");
        let step = BootstrapStep {
            step_id: "homebrew-xcode-clt-diagnostics".to_string(),
            title: "Diagnose Mac developer tools".to_string(),
            commands: vec![command.clone()],
            instruction: "Paste the diagnostic output from this Mac bootstrap check.".to_string(),
            command,
            explanation:
                "Read-only diagnostic step for macOS, Homebrew, Xcode Command Line Tools, and PATH."
                    .to_string(),
            risk: "Low: reads system version and tool paths only.".to_string(),
            expected_output:
                "macOS version, Homebrew path, Xcode Command Line Tools path, and PATH".to_string(),
            approval_required: false,
            submitted_output: None,
        };
        self.steps.push(step.clone());
        step
    }

    pub fn macos_runner_install_step(&mut self) -> BootstrapStep {
        self.push_runner_install_step(BootstrapStepSpec {
            step_id: "macos-runner-install-bootstrap",
            title: "Install the Mac runner",
            commands: vec![MACOS_RUNNER_INSTALL_COMMAND.to_string()],
            instruction: "Copy this command into Terminal on the Mac. It diagnoses network and system state before downloading the runner installer.",
            explanation: "The command first runs diagnostic checks with the system shell and curl. If the network is offline, download the macOS runner artifact on another computer from https://github.com/Hmbown/DeepSeek-TUI/releases/latest, copy it to the Mac Downloads folder, then run the copied installer locally.",
            risk: "Medium: installs the kai-runner executable on this computer after showing diagnostics. Review the downloaded file source before continuing.",
            expected_output: "Diagnostic output followed by a kai-runner version, pairing URL or code, or offline fallback instructions.",
            approval_required: true,
        })
    }

    pub fn windows_runner_install_step(&mut self) -> BootstrapStep {
        self.push_runner_install_step(BootstrapStepSpec {
            step_id: "windows-runner-install-bootstrap",
            title: "Install the Windows runner",
            commands: vec![WINDOWS_RUNNER_INSTALL_COMMAND.to_string()],
            instruction: "Copy this command into PowerShell on the Windows PC. It starts with low-risk diagnostics, then asks before any install or repair action.",
            explanation: "The command uses built-in PowerShell diagnostic checks for OS, network, winget, PowerShell execution policy, and PATH. It includes an offline fallback for downloading the Windows runner artifact on another computer from https://github.com/Hmbown/DeepSeek-TUI/releases/latest, copying it to the PC Downloads folder, and running it locally. If winget is missing or broken, the guide keeps using the direct release download and prints manual App Installer repair guidance. If execution policy or PATH looks abnormal, it prints CurrentUser-only and temporary-session recovery commands.",
            risk: "Low: diagnostics read system version, network reachability, winget status, execution policy, and PATH only. Approval required: download/start the installer, change execution policy, repair App Installer, or make any PATH change.",
            expected_output: "Low-risk diagnostic output followed by either offline fallback instructions, winget/App Installer repair guidance, execution policy/PATH guidance, a kai-runner version, or a pairing URL/code.",
            approval_required: true,
        })
    }

    fn push_runner_install_step(&mut self, spec: BootstrapStepSpec<'_>) -> BootstrapStep {
        let step = BootstrapStep {
            step_id: spec.step_id.to_string(),
            title: spec.title.to_string(),
            command: spec.commands.join("\n\n"),
            commands: spec.commands,
            instruction: spec.instruction.to_string(),
            explanation: spec.explanation.to_string(),
            risk: spec.risk.to_string(),
            expected_output: spec.expected_output.to_string(),
            approval_required: spec.approval_required,
            submitted_output: None,
        };
        self.steps.push(step.clone());
        step
    }

    pub fn submit_output(
        &mut self,
        step_id: &str,
        output: impl Into<String>,
    ) -> Result<(), BootstrapStepError> {
        let step = self
            .steps
            .iter_mut()
            .find(|step| step.step_id == step_id)
            .ok_or_else(|| BootstrapStepError::StepNotFound(step_id.to_string()))?;
        step.submitted_output = Some(output.into());
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapStepError {
    StepNotFound(String),
}

const MACOS_RUNNER_INSTALL_COMMAND: &str = r#"set -eu
# Diagnose first
echo "== Diagnose first =="
uname -a
sw_vers
command -v curl || { echo "curl is missing; use the offline download fallback."; exit 2; }
echo "Network check: github.com"
if ! curl -I --connect-timeout 10 https://github.com >/dev/null 2>&1; then
  echo "Network unavailable. Offline fallback: on another computer download the macOS kai-runner release from https://github.com/Hmbown/DeepSeek-TUI/releases/latest, copy it to ~/Downloads, then run the installer from Terminal."
  exit 2
fi
tmpdir="$(mktemp -d)"
cd "$tmpdir"
case "$(uname -m)" in
  arm64) artifact="kai-runner-aarch64-apple-darwin.tar.gz" ;;
  x86_64) artifact="kai-runner-x86_64-apple-darwin.tar.gz" ;;
  *) echo "Unsupported Mac architecture: $(uname -m)"; exit 2 ;;
esac
curl -fsSLO "https://github.com/Hmbown/DeepSeek-TUI/releases/latest/download/$artifact"
tar -xzf "$artifact"
chmod +x ./kai-runner
./kai-runner --version
./kai-runner pair"#;

const WINDOWS_RUNNER_INSTALL_COMMAND: &str = r#"$ErrorActionPreference = "Stop"
Write-Host "== Low-risk diagnostics =="
Write-Host "PowerShell $($PSVersionTable.PSVersion)"
Get-ComputerInfo -Property OsName,OsVersion,OsArchitecture
Write-Host "PowerShell execution policy:"
Get-ExecutionPolicy -List
Write-Host "PATH:"
Write-Host $env:PATH
Write-Host "winget:"
$winget = Get-Command winget -ErrorAction SilentlyContinue
if ($null -eq $winget) {
  Write-Host "winget is missing or broken. This bootstrap can continue without winget by downloading the runner directly."
  Write-Host "Manual winget fallback: open Settings > Apps > Installed apps > App Installer > Advanced options, then try Repair. If that fails, try Reset or update App Installer from Microsoft Store when online."
} else {
  Write-Host "winget path: $($winget.Source)"
  try {
    winget --version
  } catch {
    Write-Host "winget is missing or broken. Manual fallback: repair or reset App Installer in Windows Settings, then open a new PowerShell window."
  }
}
$windowsApps = Join-Path $env:LOCALAPPDATA "Microsoft\WindowsApps"
if (($env:PATH -split ';') -notcontains $windowsApps) {
  Write-Host "PATH warning: WindowsApps is not on PATH. winget and Store app shims may be hidden."
  Write-Host "Path fallback for this session only:"
  Write-Host ('$env:PATH = $env:PATH + ";' + $windowsApps + '"')
}
Write-Host "Network check: github.com:443"
$network = Test-NetConnection github.com -Port 443
if (-not $network.TcpTestSucceeded) {
  Write-Host "Offline fallback: on another computer download kai-runner-x86_64-pc-windows-msvc.exe from https://github.com/Hmbown/DeepSeek-TUI/releases/latest, copy it to this PC's Downloads folder, then run:"
  Write-Host ('& "' + (Join-Path $env:USERPROFILE "Downloads\kai-runner-x86_64-pc-windows-msvc.exe") + '" --version')
  Write-Host ('& "' + (Join-Path $env:USERPROFILE "Downloads\kai-runner-x86_64-pc-windows-msvc.exe") + '" pair')
  exit 2
}
Write-Host "== Approval required before install or repair =="
Write-Host "This next action downloads the runner .exe to Downloads and starts it. Type 'YES' to continue."
Write-Host "If PowerShell blocks local scripts later, approve only this CurrentUser repair command:"
Write-Host "Set-ExecutionPolicy -Scope CurrentUser RemoteSigned"
Write-Host "If PATH is stale, first try closing and reopening PowerShell. For a temporary session-only refresh, run:"
Write-Host '$env:PATH = [Environment]::GetEnvironmentVariable("Path", "Machine") + ";" + [Environment]::GetEnvironmentVariable("Path", "User")'
$approval = Read-Host "Type YES to download and start the Windows runner"
if ($approval -ne "YES") {
  Write-Host "No install or repair was run. Use the offline fallback or rerun this command when ready."
  exit 2
}
$download = Join-Path $env:USERPROFILE "Downloads\kai-runner-x86_64-pc-windows-msvc.exe"
Invoke-WebRequest -Uri "https://github.com/Hmbown/DeepSeek-TUI/releases/latest/download/kai-runner-x86_64-pc-windows-msvc.exe" -OutFile $download
Start-Process -FilePath $download -ArgumentList "--version" -Wait -NoNewWindow
Start-Process -FilePath $download -ArgumentList "pair" -Wait -NoNewWindow"#;
