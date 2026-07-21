[CmdletBinding()]
param([ValidateSet("codex","claude","gemini","opencode","codebuddy","cursor","generic","all")][string]$Agent="codex",[string]$SkillsDir,[string]$Source,[switch]$Force,[switch]$InstallCli,[switch]$SkipCli,[string]$ProjectDir,[string]$RootDir)
$scriptDir=Split-Path -Parent $MyInvocation.MyCommand.Path
$arguments=@("-Mode","skills-only","-OnlyScan","-SkipAgent","-Agent",$Agent)
if($SkillsDir){$arguments+=@("-SkillsDir",$SkillsDir)};if($Source){$arguments+=@("-Source",$Source)};if($ProjectDir){$arguments+=@("-ProjectDir",$ProjectDir)};if($RootDir){$arguments+=@("-RootDir",$RootDir)};if($Force){$arguments+="-Force"};if($SkipCli){$arguments+="-SkipCli"}
& (Join-Path $scriptDir "install-agent-workflow.ps1") @arguments
exit $LASTEXITCODE
