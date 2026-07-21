[CmdletBinding()]
param([ValidateSet("codex","claude","gemini","opencode","codebuddy","cursor","generic","all")][string]$Agent="codex",[string]$SkillsDir,[string]$Source,[switch]$Force,[switch]$InstallCli,[switch]$SkipCli,[string]$ProjectDir,[string]$RootDir,[Alias("h", "-help")][switch]$Help)

if ($Help) {
    @'
Usage: scripts\install-agent-skill.ps1 [options]

  -Agent NAME                   Target agent: codex, claude, gemini, opencode,
                                codebuddy, cursor, generic, or all.
  -SkillsDir DIR                Exact skills parent directory.
  -Source DIR                   Custom reforge-scan source.
  -ProjectDir DIR               Install project-local files into DIR.
  -RootDir DIR                  Override the selected agent's global root/config dir.
  -SkipCli                      Do not install the Reforge CLI.
  -InstallCli                   Install the Reforge CLI (default).
  -Force                        Atomically replace an existing installation.
  -Help, -h, --help             Print this help and exit.
'@
    exit 0
}

$scriptDir=Split-Path -Parent $MyInvocation.MyCommand.Path
$arguments=@("-Mode","skills-only","-OnlyScan","-SkipAgent","-Agent",$Agent)
if($SkillsDir){$arguments+=@("-SkillsDir",$SkillsDir)};if($Source){$arguments+=@("-Source",$Source)};if($ProjectDir){$arguments+=@("-ProjectDir",$ProjectDir)};if($RootDir){$arguments+=@("-RootDir",$RootDir)};if($Force){$arguments+="-Force"};if($SkipCli){$arguments+="-SkipCli"}
& (Join-Path $scriptDir "install-agent-workflow.ps1") @arguments
exit $LASTEXITCODE
